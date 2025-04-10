use essential_node::db::{
    self,
    pool::{Config, Source},
    ConnectionPool,
};
use essential_node_db as node_db;
use essential_node_types::{block_notify::BlockTx, Block, BlockHeader};
use essential_relayer::{DataSyncError, Relayer};
use essential_types::{
    contract::Contract,
    predicate::Predicate,
    solution::{Mutation, Solution, SolutionSet},
    PredicateAddress, Word,
};
use std::sync::Arc;
use tokio::{sync::oneshot::Sender, task::JoinHandle};

const LOCALHOST: &str = "127.0.0.1";

struct NodeServer {
    address: String,
    jh: JoinHandle<()>,
    shutdown_tx: Sender<()>,
    conn_pool: ConnectionPool,
}

#[tokio::test]
async fn test_sync() {
    let relayer_conn = new_conn_pool();

    let (node_server, source_block_tx) = test_node().await;
    let source_db = node_server.conn_pool.clone();

    let mut test_conn = relayer_conn.acquire().await.unwrap();

    node_db::with_tx(&mut test_conn, |tx| db::create_tables(tx)).unwrap();

    let (solution_sets, blocks) = test_structs();

    source_db.insert_block(blocks[0].clone()).await.unwrap();
    source_block_tx.notify();

    let relayer = Relayer::new(node_server.address.as_str()).unwrap();
    let block_tx = BlockTx::new();
    let mut block_rx = block_tx.new_listener();
    let relayer_handle = relayer.run(relayer_conn.clone(), block_tx.clone()).unwrap();

    block_rx.changed().await.unwrap();

    let result =
        node_db::with_tx_dropped(&mut test_conn, |block_tx| db::list_blocks(block_tx, 0..100))
            .unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].header.number, 0);
    assert_eq!(result[0].solution_sets.len(), 1);
    assert_eq!(result[0].solution_sets[0], solution_sets[0]);

    source_db.insert_block(blocks[1].clone()).await.unwrap();
    source_block_tx.notify();

    block_rx.changed().await.unwrap();

    let result =
        node_db::with_tx_dropped(&mut test_conn, |block_tx| db::list_blocks(block_tx, 0..100))
            .unwrap();

    assert_eq!(result.len(), 2);
    assert_eq!(result[1].header.number, 1);
    assert_eq!(result[1].solution_sets.len(), 1);
    assert_eq!(result[1].solution_sets[0], solution_sets[1]);

    relayer_handle.close().await.unwrap();

    for block in &blocks[2..] {
        source_db.insert_block(block.clone()).await.unwrap();
        source_block_tx.notify();
    }
    let relayer = Relayer::new(node_server.address.as_str()).unwrap();
    let block_tx = BlockTx::new();
    let relayer_handle = relayer.run(relayer_conn.clone(), block_tx).unwrap();

    let start = tokio::time::Instant::now();
    let mut num_solution_sets: usize = 0;
    let mut result: Vec<Block> = vec![];

    loop {
        if start.elapsed() > tokio::time::Duration::from_secs(10) {
            panic!(
                "timeout num_solution_sets: {}, {}",
                num_solution_sets,
                result.len()
            );
        }

        let tx = test_conn.transaction().unwrap();
        let Ok(r) = db::list_blocks(&tx, 0..203) else {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            continue;
        };
        drop(tx);

        result = r;
        num_solution_sets = result.iter().map(|b| b.solution_sets.len()).sum();
        if num_solution_sets >= 200 {
            break;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
    assert_eq!(num_solution_sets, 200);
    assert!(result
        .iter()
        .zip(result.iter().skip(1))
        .all(|(a, b)| a.header.number + 1 == b.header.number));

    let num_blocks = result.len();

    relayer_handle.close().await.unwrap();
    tear_down_server(node_server).await;
    drop(source_db);

    let (node_server, ..) = test_node().await;

    let relayer = Relayer::new(node_server.address.as_str()).unwrap();
    let block_tx = BlockTx::new();

    let relayer_handle = relayer.run(relayer_conn, block_tx).unwrap();
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    let r = relayer_handle.close().await;
    assert!(
        matches!(
            r,
            Err(essential_relayer::Error::DataSyncFailed(
                DataSyncError::Fork(i, _, None)
            )) if i == (num_blocks - 1) as Word
        ),
        "{} {:?}",
        num_blocks,
        r
    );
}

// Create a new AsyncConnectionPool with a unique in-memory database.
fn new_conn_pool() -> db::ConnectionPool {
    let conf = Config {
        source: Source::Memory(uuid::Uuid::new_v4().into()),
        ..Default::default()
    };
    db::ConnectionPool::with_tables(&conf).unwrap()
}

pub fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .http2_prior_knowledge() // Enforce HTTP/2
        .build()
        .unwrap()
}

async fn test_listener() -> tokio::net::TcpListener {
    tokio::net::TcpListener::bind(format!("{LOCALHOST}:0"))
        .await
        .unwrap()
}

// Spawn a test server with given ConnectionPool and block notify channel.
async fn setup_node_as_server(state: essential_node_api::State) -> NodeServer {
    let conn_pool = state.conn_pool.clone();
    let router = essential_node_api::router(state);
    let listener = test_listener().await;
    let port = listener.local_addr().unwrap().port();
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let jh = tokio::spawn(async move {
        tokio::select! {
            _ = essential_node_api::serve(&router, &listener, essential_node_api::DEFAULT_CONNECTION_LIMIT) => {},
            _ = shutdown_rx => {},
        }
    });
    let address = format!("http://{LOCALHOST}:{port}/");
    NodeServer {
        address,
        jh,
        shutdown_tx,
        conn_pool,
    }
}

// Setup node as server with a unique database of default configuration.
// Returns server and block notify channel.
async fn test_node() -> (NodeServer, BlockTx) {
    let conf = Config {
        source: Source::Memory(uuid::Uuid::new_v4().into()),
        ..Default::default()
    };
    let db = ConnectionPool::with_tables(&conf).unwrap();
    let source_block_tx = BlockTx::new();
    let source_block_rx = source_block_tx.new_listener();
    let state = essential_node_api::State {
        conn_pool: db,
        new_block: Some(source_block_rx),
    };
    let node_server = setup_node_as_server(state).await;

    (node_server, source_block_tx)
}

// Tear down the server by:
// 1. Sending a shutdown signal to the server.
// 2. Waiting for the server to shut down by awaiting the join handle.
// 3. Dropping the connection pool.
async fn tear_down_server(server: NodeServer) {
    let NodeServer {
        jh,
        shutdown_tx,
        conn_pool,
        ..
    } = server;

    shutdown_tx.send(()).unwrap();
    jh.await.unwrap();
    drop(conn_pool);
}

// Solution sets and blocks structs for testing.
fn test_structs() -> (Vec<SolutionSet>, Vec<Arc<Block>>) {
    let predicate = Predicate {
        nodes: vec![],
        edges: vec![],
    };
    let contracts: Vec<_> = (0..200)
        .map(|i| {
            Arc::new(Contract {
                predicates: vec![predicate.clone()],
                salt: [i as u8; 32],
            })
        })
        .collect();
    let solution_sets: Vec<_> = contracts
        .iter()
        .map(|c| {
            let contract = essential_hash::content_addr::<Contract>(&c.clone());
            let predicate = essential_hash::content_addr(&c.predicates[0]);
            let addr = PredicateAddress {
                contract,
                predicate,
            };
            SolutionSet {
                solutions: vec![Solution {
                    predicate_to_solve: addr,
                    predicate_data: vec![],
                    state_mutations: vec![Mutation {
                        key: vec![1],
                        value: vec![1],
                    }],
                }],
            }
        })
        .collect();
    let blocks: Vec<_> = solution_sets
        .iter()
        .enumerate()
        .map(|(i, s)| {
            Arc::new(Block {
                header: BlockHeader {
                    number: i as Word,
                    timestamp: std::time::Duration::from_secs(i as u64),
                },
                solution_sets: vec![s.clone()],
            })
        })
        .collect();

    (solution_sets, blocks)
}
