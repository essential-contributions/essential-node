use super::*;
use crate::test_utils::{self, Conn};
use essential_node_db::{
    create_tables, get_state_progress, insert_block, insert_contract, query_state,
};
use essential_types::{contract::Contract, Block, ContentAddress};
use rusqlite::Connection;
use std::time::Duration;

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

fn assert_block_progress_is_some(conn: &Connection, block: &Block, hash: &ContentAddress) {
    let (progress_number, progress_hash) = get_state_progress(conn)
        .unwrap()
        .expect("progress should be some");
    assert_eq!(progress_number, block.number);
    assert_eq!(progress_hash, *hash);
}

fn assert_block_progress_is_none(conn: &Connection) {
    assert!(get_state_progress(conn).unwrap().is_none());
}

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

    let mut conn = Conn.get().await.unwrap();

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

    let handle = block_stream(Conn, stream_rx).await.unwrap();

    assert_block_progress_is_none(&conn);

    insert_block_and_send_notification(&mut conn, &blocks[0], &stream_tx);
    tokio::time::sleep(Duration::from_millis(100)).await;
    // Check for progress and state
    assert_block_progress_is_some(&conn, &blocks[0], &hashes[0]);
    assert_multiple_block_mutations(&conn, &[&blocks[0]]).await;

    insert_block_and_send_notification(&mut conn, &blocks[1], &stream_tx);
    tokio::time::sleep(Duration::from_millis(100)).await;
    insert_block_and_send_notification(&mut conn, &blocks[2], &stream_tx);
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_block_progress_is_some(&conn, &blocks[2], &hashes[2]);
    assert_multiple_block_mutations(&conn, &[&blocks[1], &blocks[2]]).await;

    insert_block_and_send_notification(&mut conn, &blocks[3], &stream_tx);
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_block_progress_is_some(&conn, &blocks[3], &hashes[3]);
    assert_multiple_block_mutations(&conn, &[&blocks[3]]).await;

    handle.close().await.unwrap();
}

#[tokio::test]
async fn fork() {
    let mut conn = Conn.get().await.unwrap();

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

    let handle = block_stream(Conn, stream_rx).await.unwrap();

    // Stream processes block 0
    insert_block_and_send_notification(&mut conn, &blocks[0], &stream_tx);
    tokio::time::sleep(Duration::from_millis(100)).await;

    // State progress is updated outside of the stream to be block 2
    update_state_progress(&mut conn, blocks[2].number, &hashes[2]).unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Stream errors when processing block 1
    insert_block_and_send_notification(&mut conn, &blocks[1], &stream_tx);
    tokio::time::sleep(Duration::from_millis(100)).await;

    let err = handle.close().await.err().unwrap();
    assert!(matches!(err, CriticalError::Fork));
}
