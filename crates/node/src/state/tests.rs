use super::*;
use crate::test_utils::{self, test_conn_pool};
use essential_node_db::{
    create_tables, get_state_progress, insert_block, insert_contract, query_state,
};
use essential_types::{contract::Contract, Block, ContentAddress};
use rusqlite::Connection;
use std::time::Duration;

// Insert a block to the database and send a notification to the stream
fn insert_block_and_send_notification(
    conn: &mut Connection,
    block: &Block,
    stream_tx: &tokio::sync::watch::Sender<()>,
) {
    let tx = conn.transaction().unwrap();
    insert_block(&tx, block).unwrap();
    tx.commit().unwrap();
    stream_tx.send(()).unwrap();
}

// Check that the state progress in the database is block number and hash
fn assert_state_progress_is_some(conn: &Connection, block: &Block, hash: &ContentAddress) {
    let (progress_number, progress_hash) = get_state_progress(conn)
        .unwrap()
        .expect("progress should be some");
    assert_eq!(progress_number, block.number);
    assert_eq!(progress_hash, *hash);
}

// Check that the state progress in the database is none
fn assert_state_progress_is_none(conn: &Connection) {
    assert!(get_state_progress(conn).unwrap().is_none());
}

// Check state
async fn assert_multiple_block_mutations(conn: &Connection, blocks: &[&Block]) {
    for block in blocks {
        for solution in &block.solutions {
            for data in &solution.data {
                for mutation in &data.state_mutations {
                    let value = query_state(conn, &data.predicate_to_solve.contract, &mutation.key)
                        .unwrap()
                        .unwrap();
                    assert_eq!(value, mutation.value);
                }
            }
        }
    }
}

fn insert_contracts_to_db(conn: &mut Connection, contracts: Vec<Contract>) {
    let tx = conn.transaction().unwrap();
    for contract in contracts {
        insert_contract(&tx, &contract, 0).unwrap();
    }
    tx.commit().unwrap();
}

#[tokio::test]
async fn can_derive_state() {
    std::env::set_var("RUST_LOG", "trace");
    #[cfg(feature = "tracing")]
    let _ = tracing_subscriber::fmt::try_init();

    let conn_pool = test_conn_pool("can_derive_state");
    let mut conn = conn_pool.acquire().await.unwrap();

    let tx = conn.transaction().unwrap();
    create_tables(&tx).unwrap();
    tx.commit().unwrap();

    let test_blocks_count = 5;
    let (test_blocks, contracts) = test_utils::test_blocks(test_blocks_count);
    insert_contracts_to_db(&mut conn, contracts);
    tokio::time::sleep(Duration::from_millis(100)).await;

    let blocks = test_blocks;
    let hashes = blocks
        .iter()
        .map(essential_hash::content_addr)
        .collect::<Vec<_>>();

    let (stream_tx, stream_rx) = tokio::sync::watch::channel(());

    let handle = derive_state_stream(conn_pool.clone(), stream_rx)
        .await
        .unwrap();

    // Initially, the state progress is none
    assert_state_progress_is_none(&conn);

    // Process block 0
    insert_block_and_send_notification(&mut conn, &blocks[0], &stream_tx);
    tokio::time::sleep(Duration::from_millis(100)).await;
    // Assert state progress is block 0
    assert_state_progress_is_some(&conn, &blocks[0], &hashes[0]);
    // Assert mutations in block 0 are in database
    assert_multiple_block_mutations(&conn, &[&blocks[0]]).await;

    // Process block 1
    insert_block_and_send_notification(&mut conn, &blocks[1], &stream_tx);
    tokio::time::sleep(Duration::from_millis(100)).await;
    // Process block 2
    insert_block_and_send_notification(&mut conn, &blocks[2], &stream_tx);
    tokio::time::sleep(Duration::from_millis(100)).await;
    // Assert state progress is block 2
    assert_state_progress_is_some(&conn, &blocks[2], &hashes[2]);
    // Assert mutations in block 1 and 2 are in database
    assert_multiple_block_mutations(&conn, &[&blocks[1], &blocks[2]]).await;

    // Process block 3
    insert_block_and_send_notification(&mut conn, &blocks[3], &stream_tx);
    tokio::time::sleep(Duration::from_millis(100)).await;
    // Assert state progress is block 3
    assert_state_progress_is_some(&conn, &blocks[3], &hashes[3]);
    // Assert mutations in block 3 are in database
    assert_multiple_block_mutations(&conn, &[&blocks[3]]).await;

    handle.close().await.unwrap();
}

#[tokio::test]
async fn fork() {
    let conn_pool = test_conn_pool("fork");
    let mut conn = conn_pool.acquire().await.unwrap();

    let tx = conn.transaction().unwrap();
    create_tables(&tx).unwrap();
    tx.commit().unwrap();

    let test_blocks_count = 3;
    let (test_blocks, contracts) = test_utils::test_blocks(test_blocks_count);
    insert_contracts_to_db(&mut conn, contracts);
    tokio::time::sleep(Duration::from_millis(100)).await;

    let blocks = test_blocks;
    let hashes = blocks
        .iter()
        .map(essential_hash::content_addr)
        .collect::<Vec<_>>();

    let (stream_tx, stream_rx) = tokio::sync::watch::channel(());

    let handle = derive_state_stream(conn_pool.clone(), stream_rx)
        .await
        .unwrap();

    // Stream processes block 0
    insert_block_and_send_notification(&mut conn, &blocks[0], &stream_tx);
    tokio::time::sleep(Duration::from_millis(100)).await;

    // State progress is updated outside of the stream to be block 2
    update_state_progress(&conn, blocks[2].number, &hashes[2]).unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Stream errors when processing block 1
    insert_block_and_send_notification(&mut conn, &blocks[1], &stream_tx);
    tokio::time::sleep(Duration::from_millis(100)).await;

    let err = handle.close().await.err().unwrap();
    assert!(matches!(err, CriticalError::Fork));
}
