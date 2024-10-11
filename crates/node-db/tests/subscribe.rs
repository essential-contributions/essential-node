use essential_node_db::{self as node_db};
use futures::StreamExt;
use rusqlite::Connection;
use rusqlite_pool::tokio::AsyncConnectionPool;

mod util;

fn new_mem_conn(unique_id: &str) -> rusqlite::Result<Connection> {
    let conn_str = format!("file:/{unique_id}");
    let conn =
        rusqlite::Connection::open_with_flags_and_vfs(conn_str, Default::default(), "memdb")?;
    conn.pragma_update(None, "foreign_keys", true)?;
    Ok(conn)
}

struct AcquireConn(AsyncConnectionPool);

struct AwaitNewBlock(tokio::sync::watch::Receiver<()>);

impl node_db::AcquireConnection for AcquireConn {
    async fn acquire_connection(&self) -> Option<impl 'static + AsRef<Connection>> {
        self.0.acquire().await.ok()
    }
}

impl node_db::AwaitNewBlock for AwaitNewBlock {
    async fn await_new_block(&mut self) -> Option<()> {
        self.0.changed().await.ok()
    }
}

#[tokio::test]
async fn subscribe_blocks() {
    // The test blocks.
    let blocks = util::test_blocks(1000);

    // A DB connection pool.
    let conn_limit = 4;
    let conn_init = || new_mem_conn("subscribe_blocks");
    let conn_pool = AsyncConnectionPool::new(conn_limit, conn_init).unwrap();

    // A fn for notifying of new blocks.
    let (new_block_tx, new_block_rx) = tokio::sync::watch::channel(());

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
    let acq_conn = AcquireConn(conn_pool.clone());
    let new_block = AwaitNewBlock(new_block_rx);
    let stream = node_db::subscribe_blocks(start_block, acq_conn, new_block);
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
