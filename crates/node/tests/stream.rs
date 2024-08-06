use std::time::Duration;

use essential_node::stream::{block_stream, GetConn};
use essential_node_db::{create_tables, get_state_progress, get_state_value, insert_block};
use essential_types::{Block, ContentAddress};
use rusqlite::Connection;
use util::Conn;

mod util;

fn insert_and_send_block(
    conn: &mut Connection,
    block: &Block,
    stream_tx: &tokio::sync::watch::Sender<()>,
) {
    let tx = conn.transaction().unwrap();
    insert_block(&tx, block).unwrap();
    tx.commit().unwrap();
    stream_tx.send(()).unwrap();
}

fn assert_block_mutations(conn: &Connection, block: &Block, hash: &ContentAddress) {
    let (progress_number, progress_hash) = get_state_progress(&conn).unwrap().unwrap();
    assert_eq!(progress_number, block.number);
    assert_eq!(progress_hash, *hash);

    for solution in &block.solutions {
        for data in &solution.data {
            for mutation in &data.state_mutations {
                let value =
                    get_state_value(&conn, &data.predicate_to_solve.contract, &mutation.key)
                        .unwrap()
                        .unwrap();
                assert_eq!(value, mutation.value);
            }
        }
    }
}

#[tokio::test]
async fn can_derive_state() {
    let test_blocks = util::test_blocks(5);
    let blocks = test_blocks;
    let hashes = blocks
        .iter()
        .map(essential_hash::content_addr)
        .collect::<Vec<_>>();

    let mut conn = Conn.get().await.unwrap();

    let tx = conn.transaction().unwrap();
    create_tables(&tx).unwrap();
    insert_block(&tx, &blocks[0]).unwrap();
    tx.commit().unwrap();

    let (stream_tx, stream_rx) = tokio::sync::watch::channel(());

    let handle = block_stream(Conn, stream_rx).await.unwrap();

    insert_and_send_block(&mut conn, &blocks[0], &stream_tx);
    tokio::time::sleep(Duration::from_millis(100)).await;
    // Check for progress and state
    assert_block_mutations(&conn, &blocks[0], &hashes[0]);

    insert_and_send_block(&mut conn, &blocks[1], &stream_tx);
    insert_and_send_block(&mut conn, &blocks[2], &stream_tx);
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_block_mutations(&conn, &blocks[1], &hashes[1]);
    assert_block_mutations(&conn, &blocks[2], &hashes[2]);

    insert_and_send_block(&mut conn, &blocks[3], &stream_tx);
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_block_mutations(&conn, &blocks[3], &hashes[3]);

    handle.close().await.unwrap();
}
