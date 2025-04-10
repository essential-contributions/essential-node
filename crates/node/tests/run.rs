#![cfg(feature = "test-utils")]

use essential_hash::content_addr;
use essential_node::{
    self as node,
    db::{
        self,
        pool::{Config, Source},
        ConnectionPool,
    },
    test_utils::{
        assert_multiple_block_mutations, assert_validation_progress_is_some,
        test_blocks_with_contracts, test_db_conf,
    },
    RunConfig,
};
use essential_node_db as node_db;
use essential_node_types::{block_notify::BlockTx, BigBang, Block};
use rusqlite::Connection;

const LOCALHOST: &str = "127.0.0.1";

struct NodeServer {
    address: String,
    conn_pool: ConnectionPool,
}

#[tokio::test]
async fn test_run() {
    let (node_server, source_block_tx) = test_node().await;

    // Setup node
    let conf = test_db_conf();
    let db = ConnectionPool::with_tables(&conf).unwrap();
    let big_bang = BigBang::default();
    node::ensure_big_bang_block(&db, &big_bang).await.unwrap();

    // Run node
    let block_tx = BlockTx::new();
    let run_conf = RunConfig {
        relayer_source_endpoint: Some(node_server.address),
        run_validation: true,
    };
    let big_bang = BigBang::default();
    let _handle = node::run(
        db.clone(),
        run_conf,
        big_bang.contract_registry.contract.clone(),
        big_bang.program_registry.contract.clone(),
        block_tx,
    )
    .unwrap();

    // Create test blocks
    let test_blocks_count = 4;
    let test_blocks = test_blocks_with_contracts(1, test_blocks_count + 1);
    let conn = db.acquire().await.unwrap();

    // Initially, the validation progress is big bang block
    let bbb_ca = essential_hash::content_addr(&big_bang.block());
    assert_validation_progress_is_some(&conn, &bbb_ca);

    // Insert block 0 to database and send notification
    node_server
        .conn_pool
        .insert_block(test_blocks[0].clone().into())
        .await
        .unwrap();
    source_block_tx.notify();
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Check block, state and validation progress
    let mut conn = db.acquire().await.unwrap();
    assert_submit_block_effects(&mut conn, vec![test_blocks[0].clone()]);

    // Insert block 1 and 2 to database and send notification
    node_server
        .conn_pool
        .insert_block(test_blocks[1].clone().into())
        .await
        .unwrap();
    source_block_tx.notify();
    node_server
        .conn_pool
        .insert_block(test_blocks[2].clone().into())
        .await
        .unwrap();
    source_block_tx.notify();
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Check block, state and validation progress
    let mut conn = db.acquire().await.unwrap();
    assert_submit_block_effects(
        &mut conn,
        vec![test_blocks[1].clone(), test_blocks[2].clone()],
    );

    // Insert block 3 to database and send notification
    node_server
        .conn_pool
        .insert_block(test_blocks[3].clone().into())
        .await
        .unwrap();
    source_block_tx.notify();
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Check block, state and validation progress
    let mut conn = db.acquire().await.unwrap();
    assert_submit_block_effects(&mut conn, vec![test_blocks[3].clone()]);
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
    let _jh = tokio::spawn(async move {
        essential_node_api::serve(
            &router,
            &listener,
            essential_node_api::DEFAULT_CONNECTION_LIMIT,
        )
        .await
    });
    let address = format!("http://{LOCALHOST}:{port}/");
    NodeServer { address, conn_pool }
}

// Setup node as server with a unique database of default configuration.
// Returns server and block notify channel.
async fn test_node() -> (NodeServer, BlockTx) {
    let conf = Config {
        source: Source::Memory(uuid::Uuid::new_v4().into()),
        ..Default::default()
    };
    let db = ConnectionPool::with_tables(&conf).unwrap();
    let big_bang = BigBang::default();
    node::ensure_big_bang_block(&db, &big_bang).await.unwrap();
    let source_block_tx = BlockTx::new();
    let source_block_rx = source_block_tx.new_listener();
    let state = essential_node_api::State {
        conn_pool: db,
        new_block: Some(source_block_rx),
    };
    let node_server = setup_node_as_server(state).await;

    (node_server, source_block_tx)
}

// Fetch blocks from node database and assert that they contain the same solution sets as expected.
// Assert state mutations in the blocks have been applied to database.
// Assert validation progress is the latest fetched block.
fn assert_submit_block_effects(conn: &mut Connection, expected_blocks: Vec<Block>) {
    let fetched_blocks = node_db::with_tx_dropped(conn, |tx| {
        db::list_blocks(
            tx,
            expected_blocks[0].header.number
                ..expected_blocks[expected_blocks.len() - 1].header.number + 1,
        )
    })
    .unwrap();

    for (i, expected_block) in expected_blocks.iter().enumerate() {
        // Check if the block was added to the database
        assert_eq!(
            fetched_blocks[i].header.number,
            expected_block.header.number
        );
        assert_eq!(
            fetched_blocks[i].solution_sets.len(),
            expected_block.solution_sets.len()
        );
        for (j, fetched_block_solution_set) in fetched_blocks[i].solution_sets.iter().enumerate() {
            assert_eq!(
                fetched_block_solution_set,
                &expected_block.solution_sets[j].clone()
            )
        }
        // Assert mutations in block are in database
        assert_multiple_block_mutations(conn, &[&fetched_blocks[i]]);
    }
    // Assert validation progress is latest block
    assert_validation_progress_is_some(
        conn,
        &content_addr(&fetched_blocks[fetched_blocks.len() - 1]),
    );
}
