use essential_node::stream::{block_stream, GetConn};
use essential_node_db::{create_tables, get_state_progress, insert_block, query_state};
use essential_types::{Block, ContentAddress};
use rusqlite::Connection;
use std::time::Duration;
use util::Conn;

mod util;

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

fn assert_block_progress(conn: &Connection, block: &Block, hash: &ContentAddress) {
    match get_state_progress(conn).unwrap() {
        Some((progress_number, progress_hash)) => {
            assert_eq!(progress_number, block.number);
            assert_eq!(progress_hash, *hash);
        }
        None => {
            assert_eq!(block.number, 0);
        }
    }
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

#[tokio::test]
async fn can_derive_state() {
    std::env::set_var("RUST_LOG", "trace");
    #[cfg(feature = "tracing")]
    let _ = tracing_subscriber::fmt::try_init();

    let mut conn = Conn.get().await.unwrap();

    let tx = conn.transaction().unwrap();
    create_tables(&tx).unwrap();
    tx.commit().unwrap();

    let test_blocks = util::test_blocks(&mut Some(&mut conn), 5);
    let blocks = test_blocks;
    let hashes = blocks
        .iter()
        .map(essential_hash::content_addr)
        .collect::<Vec<_>>();
    tokio::time::sleep(Duration::from_millis(100)).await;

    let (stream_tx, stream_rx) = tokio::sync::watch::channel(());

    let handle = block_stream(Conn, stream_rx).await.unwrap();

    insert_block_and_send_notification(&mut conn, &blocks[0], &stream_tx);
    tokio::time::sleep(Duration::from_millis(100)).await;
    // Check for progress and state
    assert_block_progress(&conn, &blocks[0], &hashes[0]);
    assert_multiple_block_mutations(&conn, &[&blocks[0]]).await;

    insert_block_and_send_notification(&mut conn, &blocks[1], &stream_tx);
    tokio::time::sleep(Duration::from_millis(100)).await;
    insert_block_and_send_notification(&mut conn, &blocks[2], &stream_tx);
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_block_progress(&conn, &blocks[2], &hashes[2]);
    assert_multiple_block_mutations(&conn, &[&blocks[1], &blocks[2]]).await;

    insert_block_and_send_notification(&mut conn, &blocks[3], &stream_tx);
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_block_progress(&conn, &blocks[3], &hashes[3]);
    assert_multiple_block_mutations(&conn, &[&blocks[3]]).await;

    handle.close().await.unwrap();
}
