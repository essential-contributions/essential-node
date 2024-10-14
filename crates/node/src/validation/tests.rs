use super::*;
use crate::test_utils::{
    assert_validation_progress_is_none, assert_validation_progress_is_some, test_blocks,
    test_conn_pool, test_invalid_block,
};
use essential_node_db::{insert_block, insert_contract};
use essential_types::{contract::Contract, Block, Word};
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

    let conn_pool = test_conn_pool();
    let mut conn = conn_pool.acquire().await.unwrap();

    const NUM_TEST_BLOCKS: Word = 4;
    let (test_blocks, contracts) = test_blocks(NUM_TEST_BLOCKS);
    insert_contracts_to_db(&mut conn, contracts);

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

#[tokio::test]
async fn test_invalid_block_validation() {
    let conn_pool = test_conn_pool();
    let mut conn = conn_pool.acquire().await.unwrap();

    let (block, contract) = test_invalid_block(0, Duration::from_secs(0));
    insert_contracts_to_db(&mut conn, vec![contract]);

    let (block_tx, block_rx) = tokio::sync::watch::channel(());

    let handle = validation_stream(conn_pool.clone(), block_rx).unwrap();

    // Initially, the validation progress is none
    assert_validation_progress_is_none(&conn);

    // Process invalid block
    insert_block_and_send_notification(&mut conn, &block, &block_tx);
    tokio::time::sleep(Duration::from_millis(100)).await;
    // Assert validation progress is still none
    assert_validation_progress_is_none(&conn);
    // Assert block is in failed blocks table
    let fetched_failed_blocks = essential_node_db::list_failed_blocks(&conn, 0..10).unwrap();
    assert_eq!(fetched_failed_blocks.len(), 1);
    assert_eq!(fetched_failed_blocks[0].0, block.number);
    assert_eq!(
        fetched_failed_blocks[0].1,
        content_addr(&block.solutions[0])
    );

    handle.close().await.unwrap();
}

#[tokio::test]
async fn can_process_valid_and_invalid_blocks() {
    #[cfg(feature = "tracing")]
    let _ = tracing_subscriber::fmt::try_init();

    let conn_pool = test_conn_pool();
    let mut conn = conn_pool.acquire().await.unwrap();

    // Two valid blocks with number 0 and 1
    let (test_blocks, mut contracts) = test_blocks(2);
    // One invalid block with number 1
    let (invalid_block, contract) = test_invalid_block(1, Duration::from_secs(1));
    contracts.push(contract);
    insert_contracts_to_db(&mut conn, contracts);

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

    // Process invalid block
    insert_block_and_send_notification(&mut conn, &invalid_block, &block_tx);
    tokio::time::sleep(Duration::from_millis(100)).await;
    // Assert validation progress is still block 0
    assert_validation_progress_is_some(&conn, &hashes[0]);
    // Assert block is in failed blocks table
    let fetched_failed_blocks = essential_node_db::list_failed_blocks(&conn, 0..10).unwrap();
    assert_eq!(fetched_failed_blocks.len(), 1);
    assert_eq!(fetched_failed_blocks[0].0, invalid_block.number);
    assert_eq!(
        fetched_failed_blocks[0].1,
        content_addr(&invalid_block.solutions[0])
    );

    // Process block 1
    insert_block_and_send_notification(&mut conn, &blocks[1], &block_tx);
    tokio::time::sleep(Duration::from_millis(100)).await;
    // Assert validation progress is block 1
    assert_validation_progress_is_some(&conn, &hashes[1]);

    handle.close().await.unwrap();
}
