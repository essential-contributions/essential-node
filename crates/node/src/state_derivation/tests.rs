use super::*;
use crate::test_utils::{
    self, assert_multiple_block_mutations, assert_state_progress_is_none,
    assert_state_progress_is_some, test_conn_pool,
};
use essential_node_db::insert_block;
use essential_types::Block;
use rusqlite::Connection;
use std::time::Duration;

// Insert a block to the database and send a notification to the stream
fn insert_block_and_send_notification(
    conn: &mut Connection,
    block: &Block,
    state_tx: &tokio::sync::watch::Sender<()>,
) {
    let tx = conn.transaction().unwrap();
    insert_block(&tx, block).unwrap();
    tx.commit().unwrap();
    state_tx.send(()).unwrap();
}

#[tokio::test]
async fn can_derive_state() {
    #[cfg(feature = "tracing")]
    let _ = tracing_subscriber::fmt::try_init();

    let conn_pool = test_conn_pool();
    let mut conn = conn_pool.acquire().await.unwrap();

    let test_blocks_count = 4;
    let test_blocks = test_utils::test_blocks_with_contracts(0, test_blocks_count);

    let blocks = test_blocks;
    let hashes = blocks.iter().map(content_addr).collect::<Vec<_>>();

    let (state_tx, state_rx) = tokio::sync::watch::channel(());

    let handle = state_derivation_stream(conn_pool.clone(), state_rx).unwrap();

    // Initially, the state progress is none
    assert_state_progress_is_none(&conn);

    // Process block 0
    insert_block_and_send_notification(&mut conn, &blocks[0], &state_tx);
    tokio::time::sleep(Duration::from_millis(100)).await;
    // Assert state progress is block 0
    assert_state_progress_is_some(&conn, &hashes[0]);
    // Assert mutations in block 0 are in database
    assert_multiple_block_mutations(&conn, &[&blocks[0]]);

    // Process block 1
    insert_block_and_send_notification(&mut conn, &blocks[1], &state_tx);
    tokio::time::sleep(Duration::from_millis(100)).await;
    // Process block 2
    insert_block_and_send_notification(&mut conn, &blocks[2], &state_tx);
    tokio::time::sleep(Duration::from_millis(100)).await;
    // Assert state progress is block 2
    assert_state_progress_is_some(&conn, &hashes[2]);
    // Assert mutations in block 1 and 2 are in database
    assert_multiple_block_mutations(&conn, &[&blocks[1], &blocks[2]]);

    // Process block 3
    insert_block_and_send_notification(&mut conn, &blocks[3], &state_tx);
    tokio::time::sleep(Duration::from_millis(100)).await;
    // Assert state progress is block 3
    assert_state_progress_is_some(&conn, &hashes[3]);
    // Assert mutations in block 3 are in database
    assert_multiple_block_mutations(&conn, &[&blocks[3]]);

    handle.close().await.unwrap();
}
