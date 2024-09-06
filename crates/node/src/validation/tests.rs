use super::*;
use crate::test_utils::{
    assert_validation_progress_is_none, assert_validation_progress_is_some, test_blocks,
    test_conn_pool,
};
use essential_node_db::{create_tables, insert_block, insert_contract};
use essential_types::{contract::Contract, Block};
use rusqlite::Connection;
use std::time::Duration;

// Insert a block to the database and send a notification to the stream
fn insert_block_and_send_notification(
    conn: &mut Connection,
    block: &Block,
    state_rx: &tokio::sync::watch::Sender<()>,
) {
    let tx = conn.transaction().unwrap();
    insert_block(&tx, block).unwrap();
    tx.commit().unwrap();
    state_rx.send(()).unwrap();
}

fn insert_contracts_to_db(conn: &mut Connection, contracts: Vec<Contract>) {
    let tx = conn.transaction().unwrap();
    for contract in contracts {
        insert_contract(&tx, &contract, 0).unwrap();
    }
    tx.commit().unwrap();
}

#[tokio::test]
async fn can_validate() {
    #[cfg(feature = "tracing")]
    let _ = tracing_subscriber::fmt::try_init();

    let conn_pool = test_conn_pool("can_validate");
    let mut conn = conn_pool.acquire().await.unwrap();

    let tx = conn.transaction().unwrap();
    create_tables(&tx).unwrap();
    tx.commit().unwrap();

    let test_blocks_count = 4;
    let (test_blocks, contracts) = test_blocks(test_blocks_count);
    insert_contracts_to_db(&mut conn, contracts);
    tokio::time::sleep(Duration::from_millis(100)).await;

    let blocks = test_blocks;
    let hashes = blocks.iter().map(content_addr).collect::<Vec<_>>();

    let (block_tx, block_rx) = tokio::sync::watch::channel(());

    let handle = validation_stream(conn_pool.clone(), block_rx).unwrap();

    // Initially, the validation progress is none
    assert_validation_progress_is_none(&conn);

    // Process block 0
    insert_block_and_send_notification(&mut conn, &blocks[0], &block_tx);
    tokio::time::sleep(Duration::from_millis(100)).await;
    // Assert validation progress is block 0
    assert_validation_progress_is_some(&conn, &hashes[0]);

    // Process block 1
    insert_block_and_send_notification(&mut conn, &blocks[1], &block_tx);
    tokio::time::sleep(Duration::from_millis(100)).await;
    // Process block 2
    insert_block_and_send_notification(&mut conn, &blocks[2], &block_tx);
    tokio::time::sleep(Duration::from_millis(100)).await;
    // Assert validation progress is block 2
    assert_validation_progress_is_some(&conn, &hashes[2]);

    // Process block 3
    insert_block_and_send_notification(&mut conn, &blocks[3], &block_tx);
    tokio::time::sleep(Duration::from_millis(100)).await;
    // Assert validation progress is block 3
    assert_validation_progress_is_some(&conn, &hashes[3]);

    handle.close().await.unwrap();
}
