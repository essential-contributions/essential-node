use essential_node_db::{self as node_db};
use futures::StreamExt;
use rusqlite::Connection;
use rusqlite_pool::tokio::AsyncConnectionPool;

mod util;

fn new_mem_conn(unique_id: &str) -> rusqlite::Result<Connection> {
    let conn_str = format!("file:/{unique_id}");
    rusqlite::Connection::open_with_flags_and_vfs(conn_str, Default::default(), "memdb")
}

#[tokio::test]
async fn subscribe_blocks() {
    // The test blocks.
    let blocks = util::test_blocks(1000);

    // A DB connection pool.
    let conn_limit = 4;
    let conn_init = || new_mem_conn("subscribe_blocks");
    let conn_pool = AsyncConnectionPool::new(conn_limit, conn_init).unwrap();

    // A fn to acquire a connection.
    let conn_pool2 = conn_pool.clone();
    let acquire_conn = move || {
        let conn_pool = conn_pool2.clone();
        async move { conn_pool.clone().acquire().await.ok() }
    };

    // A fn for notifying of new blocks.
    let (new_block_tx, new_block_rx) = tokio::sync::watch::channel(());
    let new_block_rx = std::sync::Arc::new(tokio::sync::Mutex::new(new_block_rx));
    let await_new_block = move || {
        let rx = new_block_rx.clone();
        async move { rx.lock().await.changed().await.ok() }
    };

    // Write the first 10 blocks to the DB. We'll write the rest later.
    let mut conn = conn_pool.acquire().await.unwrap();
    let tx = conn.transaction().unwrap();
    node_db::create_tables(&tx).unwrap();
    for block in &blocks[..10] {
        node_db::insert_block(&tx, block).unwrap();
    }
    tx.commit().unwrap();
    std::mem::drop(conn);

    // Subscribe to blocks.
    let start_block = 0;
    let stream = node_db::subscribe_blocks(start_block, acquire_conn, await_new_block);
    let mut stream = std::pin::pin!(stream);

    // There should be 10 blocks available already.
    let fetched_blocks: Vec<_> = stream.by_ref().take(10).map(Result::unwrap).collect().await;
    assert_eq!(&blocks[..10], &fetched_blocks);

    // Write the remaining blocks asynchronously.
    let blocks_remaining = blocks[10..].to_vec();
    let jh = tokio::spawn(async move {
        for block in blocks_remaining {
            let mut conn = conn_pool.acquire().await.unwrap();
            let tx = conn.transaction().unwrap();
            node_db::insert_block(&tx, &block).unwrap();
            tx.commit().unwrap();
            new_block_tx.clone().send(()).unwrap();
        }
        // After writing, drop the connection pool and new block tx.
        assert_eq!(new_block_tx.receiver_count(), 1);
        std::mem::drop(new_block_tx);
    });

    // The stream should yield the remaining 90 blocks and then complete after
    // the `new_block_tx` drops.
    let fetched_blocks: Vec<_> = stream.map(Result::unwrap).collect().await;
    assert_eq!(&blocks[10..], &fetched_blocks);

    jh.await.unwrap();
}
