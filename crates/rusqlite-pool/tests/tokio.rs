#![cfg(feature = "tokio")]

use rusqlite::{Connection, Result};
use rusqlite_pool::tokio::AsyncConnectionPool;
use std::sync::Arc;

// Allow for providing an ID to make sure each test gets its own DB.
fn new_mem_conn(unique_id: &str) -> Result<Connection> {
    let flags = rusqlite::OpenFlags::default()
        | rusqlite::OpenFlags::SQLITE_OPEN_SHARED_CACHE
        | rusqlite::OpenFlags::SQLITE_OPEN_MEMORY;
    let conn_str = format!("file:{unique_id}");
    rusqlite::Connection::open_with_flags(conn_str, flags)
}

#[tokio::test]
async fn test_async_pool_congestion() {
    let new_conn = || new_mem_conn("test_async_pool_congestion");
    let pool = Arc::new(AsyncConnectionPool::new(5, new_conn).unwrap());

    let mut handles = vec![];

    // Spawn a 1000 connections.
    for _ in 0..1000 {
        let pool = pool.clone();
        let handle = tokio::spawn(async move {
            if let Ok(handle) = pool.acquire().await {
                // Simulate some work with the connection.
                handle
                    .execute("CREATE TABLE IF NOT EXISTS test (id INTEGER)", ())
                    .unwrap();
                handle
                    .execute("INSERT INTO test (id) VALUES (?)", [1])
                    .unwrap();

                let mut stmt = handle.prepare("SELECT id FROM test").unwrap();
                let _id: i32 = stmt.query_row((), |row| row.get(0)).unwrap();
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await.unwrap();
    }

    assert!(pool.all_connections_ready());
}

#[tokio::test]
async fn test_create_async_pool() {
    let new_conn = || new_mem_conn("test_create_async_pool");
    let pool = AsyncConnectionPool::new(5, new_conn).unwrap();
    assert_eq!(pool.capacity(), 5);
    assert!(pool.all_connections_ready());
}

#[tokio::test]
async fn test_acquire_async_connection() {
    let new_conn = || new_mem_conn("test_acquire_async_connection");
    let pool = AsyncConnectionPool::new(3, new_conn).unwrap();

    let handle = pool.acquire().await.unwrap();
    assert!(!pool.all_connections_ready());

    drop(handle);
    assert!(pool.all_connections_ready());
}

#[tokio::test]
async fn test_acquire_all_async_connections() {
    let new_conn = || new_mem_conn("test_acquire_all_async_connections");
    let pool = AsyncConnectionPool::new(2, new_conn).unwrap();

    let handle1 = pool.acquire().await.unwrap();
    let handle2 = pool.acquire().await.unwrap();

    assert!(pool.try_acquire().is_err());

    drop(handle1);
    assert!(pool.acquire().await.is_ok());

    drop(handle2);
    assert!(pool.all_connections_ready());
}

#[tokio::test]
async fn test_use_async_connections() {
    let new_conn = || new_mem_conn("test_use_async_connections");
    let pool = AsyncConnectionPool::new(3, new_conn).unwrap();

    // Create the table with one handle.
    let handle1 = pool.acquire().await.unwrap();
    handle1
        .execute("CREATE TABLE test (id INTEGER)", ())
        .unwrap();

    // Insert into the table with another handle.
    let handle2 = pool.acquire().await.unwrap();
    handle2
        .execute("INSERT INTO test (id) VALUES (?)", [42])
        .unwrap();

    // Query from another handle.
    let handle3 = pool.acquire().await.unwrap();
    {
        let mut stmt = handle3.prepare("SELECT id FROM test").unwrap();
        let id: i32 = stmt.query_row((), |row| row.get(0)).unwrap();
        assert_eq!(id, 42);
    }

    // No connections left.
    assert!(pool.try_acquire().is_err());

    // Return all connections.
    std::mem::drop((handle1, handle2, handle3));
    assert!(pool.all_connections_ready());
}

#[tokio::test]
async fn test_close_async_pool() {
    let new_conn = || new_mem_conn("test_close_async_pool");
    let pool = AsyncConnectionPool::new(3, new_conn).unwrap();

    let handle = pool.acquire().await.unwrap();
    drop(handle);

    let close_results = pool.close().await;
    assert_eq!(close_results.len(), 3);
    for result in close_results {
        assert!(result.is_ok());
    }
}
