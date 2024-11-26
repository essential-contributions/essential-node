use essential_node_db::{self as node_db, QueryError};
use essential_node_types::block_notify::BlockTx;
use futures::StreamExt;
use util::test_conn_pool;

mod util;

#[tokio::test]
async fn subscribe_blocks() {
    // The test blocks.
    let blocks = util::test_blocks(1000);

    // A DB connection pool.
    let conn_pool = test_conn_pool();

    // Channel for notifying of new blocks
    let new_block_tx = BlockTx::new();
    let new_block_rx = new_block_tx.new_listener();

    // Write the first 10 blocks to the DB. We'll write the rest later.
    let mut conn = conn_pool.acquire().await.unwrap();
    node_db::with_tx::<_, QueryError>(&mut conn, |tx| {
        node_db::create_tables(tx).unwrap();
        for block in &blocks[..10] {
            node_db::insert_block(tx, block).unwrap();
        }
        Ok(())
    })
    .unwrap();

    std::mem::drop(conn);

    // Subscribe to blocks.
    let start_block = 0;
    let stream = node_db::subscribe_blocks(start_block, conn_pool.clone(), new_block_rx);
    let mut stream = std::pin::pin!(stream);

    // There should be 10 blocks available already.
    let fetched_blocks: Vec<_> = stream.by_ref().take(10).map(Result::unwrap).collect().await;
    assert_eq!(&blocks[..10], &fetched_blocks);

    // Write the remaining blocks asynchronously.
    let blocks_remaining = blocks[10..].to_vec();
    let jh = tokio::spawn(async move {
        for block in blocks_remaining {
            let mut conn = conn_pool.acquire().await.unwrap();
            node_db::with_tx::<_, QueryError>(&mut conn, |tx| {
                node_db::insert_block(tx, &block).unwrap();
                Ok(())
            })
            .unwrap();

            new_block_tx.notify();
        }
        // After writing, drop the new block tx, closing the stream.
        assert_eq!(new_block_tx.receiver_count(), 1);
        std::mem::drop(new_block_tx);
    });

    // The stream should yield the remaining blocks and then complete after the
    // `new_block_tx` drops.
    let fetched_blocks: Vec<_> = stream.map(Result::unwrap).collect().await;
    assert_eq!(&blocks[10..], &fetched_blocks);

    jh.await.unwrap();
}
