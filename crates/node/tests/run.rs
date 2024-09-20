#![cfg(feature = "test-utils")]

use essential_hash::content_addr;
use essential_node::{
    db::{Config as DbConfig, ConnectionPool},
    test_utils::{
        assert_multiple_block_mutations, assert_state_progress_is_none,
        assert_state_progress_is_some, assert_validation_progress_is_none,
        assert_validation_progress_is_some, test_blocks, test_db_conf,
    },
    Node,
};
use essential_types::Block;
use rusqlite::Connection;
use std::sync::Arc;

const LOCALHOST: &str = "127.0.0.1";

struct NodeServer {
    address: String,
    conn_pool: ConnectionPool,
}

#[tokio::test]
async fn test_run() {
    #[cfg(feature = "tracing")]
    let _ = tracing_subscriber::fmt::try_init();

    let (node_server, source_block_notify) = test_node().await;

    // Setup node
    let conf = test_db_conf();
    let node = Node::new(&conf).unwrap();

    // Run node
    let db = node.db();
    let _handle = node.run(node_server.address).unwrap();

    // Create test blocks
    let test_blocks_count = 4;
    let (test_blocks, test_contracts) = test_blocks(test_blocks_count);

    // Insert contracts to database
    for contract in &test_contracts {
        db.insert_contract(Arc::new(contract.clone()), 0)
            .await
            .unwrap();
    }
    let conn = db.acquire().await.unwrap();

    // Initially, the state progress and validation progress are none
    assert_state_progress_is_none(&conn);
    assert_validation_progress_is_none(&conn);

    // Insert block 0 to database and send notification
    node_server
        .conn_pool
        .insert_block(test_blocks[0].clone().into())
        .await
        .unwrap();
    source_block_notify.clone().send(()).unwrap();
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Check block, state and state progress
    let conn = db.acquire().await.unwrap();
    assert_submit_solutions_effects(&conn, vec![test_blocks[0].clone()]);

    // Insert block 1 and 2 to database and send notification
    node_server
        .conn_pool
        .insert_block(test_blocks[1].clone().into())
        .await
        .unwrap();
    source_block_notify.clone().send(()).unwrap();
    node_server
        .conn_pool
        .insert_block(test_blocks[2].clone().into())
        .await
        .unwrap();
    source_block_notify.clone().send(()).unwrap();
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Check block, state and state progress
    let conn = db.acquire().await.unwrap();
    assert_submit_solutions_effects(&conn, vec![test_blocks[1].clone(), test_blocks[2].clone()]);

    // Insert block 3 to database and send notification
    node_server
        .conn_pool
        .insert_block(test_blocks[3].clone().into())
        .await
        .unwrap();
    source_block_notify.clone().send(()).unwrap();
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Check block, state and state progress
    let conn = db.acquire().await.unwrap();
    assert_submit_solutions_effects(&conn, vec![test_blocks[3].clone()]);
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
async fn test_node() -> (NodeServer, tokio::sync::watch::Sender<()>) {
    let node_conf = essential_node::Config {
        db: DbConfig {
            conn_limit: essential_node::db::Config::default_conn_limit(),
            source: essential_node::db::Source::Memory(uuid::Uuid::new_v4().to_string()),
        },
    };
    let node = Node::new(&node_conf).unwrap();
    let source_db = node.db();
    let (source_block_notify, node_new_block) = tokio::sync::watch::channel(());
    let state = essential_node_api::State {
        conn_pool: source_db.clone(),
        new_block: Some(node_new_block.clone()),
    };
    let node_server = setup_node_as_server(state.clone()).await;

    (node_server, source_block_notify)
}

// Fetch blocks from node database and assert that they contain the same solutions as expected.
// Assert state mutations in the blocks have been applied to database.
// Assert state progress is the latest fetched block.
// Assert validation progress is the latest fetched block.
fn assert_submit_solutions_effects(conn: &Connection, expected_blocks: Vec<Block>) {
    let fetched_blocks = &essential_node_db::list_blocks(
        conn,
        expected_blocks[0].number..expected_blocks[expected_blocks.len() - 1].number + 1,
    )
    .unwrap();
    for (i, expected_block) in expected_blocks.iter().enumerate() {
        // Check if the block was added to the database
        assert_eq!(fetched_blocks[i].number, expected_block.number);
        assert_eq!(
            fetched_blocks[i].solutions.len(),
            expected_block.solutions.len()
        );
        for (j, fetched_block_solution) in fetched_blocks[i].solutions.iter().enumerate() {
            assert_eq!(fetched_block_solution, &expected_block.solutions[j].clone())
        }
        // Assert mutations in block are in database
        assert_multiple_block_mutations(conn, &[&fetched_blocks[i]]);
    }
    // Assert state progress is latest block
    assert_state_progress_is_some(
        conn,
        &content_addr(&fetched_blocks[fetched_blocks.len() - 1]),
    );
    // Assert validation progress is latest block
    assert_validation_progress_is_some(
        conn,
        &content_addr(&fetched_blocks[fetched_blocks.len() - 1]),
    );
}
