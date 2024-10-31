use super::*;
use crate::{
    db::{self, finalize_block, insert_block},
    test_utils::{
        assert_validation_progress_is_some, test_blocks_with_contracts,
        test_conn_pool_with_big_bang, test_contract_registry, test_invalid_block_with_contract,
    },
};
use essential_node_types::{BigBang, BlockTx};
use essential_types::{Block, Word};
use rusqlite::Connection;
use std::time::Duration;

// Insert a block to the database and send a notification to the stream
fn insert_block_and_send_notification(conn: &mut Connection, block: &Block, block_tx: &BlockTx) {
    let tx = conn.transaction().unwrap();
    let block_ca = insert_block(&tx, block).unwrap();
    finalize_block(&tx, &block_ca).unwrap();
    tx.commit().unwrap();
    block_tx.notify();
}

#[tokio::test]
async fn can_validate() {
    #[cfg(feature = "tracing")]
    let _ = tracing_subscriber::fmt::try_init();

    let conn_pool = test_conn_pool_with_big_bang().await;
    let mut conn = conn_pool.acquire().await.unwrap();

    const NUM_TEST_BLOCKS: Word = 4;
    let blocks = test_blocks_with_contracts(1, 1 + NUM_TEST_BLOCKS);
    let hashes = blocks.iter().map(content_addr).collect::<Vec<_>>();

    let block_tx = BlockTx::new();
    let block_rx = block_tx.new_listener();

    let contract_registry = test_contract_registry().contract;
    let handle = validation_stream(conn_pool.clone(), contract_registry, block_rx).unwrap();

    // Initially, the validation progress is the big bang block.
    let bbb_ca = essential_hash::content_addr(&BigBang::default().block());
    assert_validation_progress_is_some(&conn, &bbb_ca);

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
    let conn_pool = test_conn_pool_with_big_bang().await;
    let mut conn = conn_pool.acquire().await.unwrap();

    let block = test_invalid_block_with_contract(1, Duration::from_secs(1));

    let block_tx = BlockTx::new();
    let block_rx = block_tx.new_listener();

    let contract_registry = test_contract_registry().contract;
    let handle = validation_stream(conn_pool.clone(), contract_registry, block_rx).unwrap();

    // Initially, the validation progress starts from the big bang block.
    let bbb_ca = essential_hash::content_addr(&BigBang::default().block());
    assert_validation_progress_is_some(&conn, &bbb_ca);

    // Process invalid block
    insert_block_and_send_notification(&mut conn, &block, &block_tx);
    tokio::time::sleep(Duration::from_millis(100)).await;
    // Assert validation progress is still BBB.
    assert_validation_progress_is_some(&conn, &bbb_ca);
    // Assert block is in failed blocks table
    let fetched_failed_blocks = db::list_failed_blocks(&conn, 0..10).unwrap();
    assert_eq!(fetched_failed_blocks.len(), 1);
    assert_eq!(fetched_failed_blocks[0].0, block.number);
    assert_eq!(
        fetched_failed_blocks[0].1,
        content_addr(&block.solutions[0])
    );

    handle.close().await.unwrap();
}

// NOTE: Temporarily ignore until issue `#100` is resolved as
//       this test requires the ability to query non-finalized blocks.
#[ignore]
#[tokio::test]
async fn can_process_valid_and_invalid_blocks() {
    #[cfg(feature = "tracing")]
    let _ = tracing_subscriber::fmt::try_init();

    let conn_pool = test_conn_pool_with_big_bang().await;
    let mut conn = conn_pool.acquire().await.unwrap();

    // Two valid blocks with number 1 and 2
    let test_blocks = test_blocks_with_contracts(1, 3);
    // One invalid block with number 2
    let invalid_block = test_invalid_block_with_contract(2, Duration::from_secs(2));

    let blocks = test_blocks;
    let hashes = blocks.iter().map(content_addr).collect::<Vec<_>>();

    let block_tx = BlockTx::new();
    let block_rx = block_tx.new_listener();

    let contract_registry = test_contract_registry().contract;
    let handle = validation_stream(conn_pool.clone(), contract_registry, block_rx).unwrap();

    // Initially, the validation progress is none
    let bbb_ca = essential_hash::content_addr(&BigBang::default().block());
    assert_validation_progress_is_some(&conn, &bbb_ca);

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
    let fetched_failed_blocks = db::list_failed_blocks(&conn, 0..10).unwrap();
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
