use rusqlite::Connection;
use rusqlite_pool::ConnectionPool;

// Allow for providing an ID to make sure each test gets its own DB.
fn new_mem_conn(unique_id: &str) -> rusqlite::Result<Connection> {
    let flags = rusqlite::OpenFlags::default()
        | rusqlite::OpenFlags::SQLITE_OPEN_SHARED_CACHE
        | rusqlite::OpenFlags::SQLITE_OPEN_MEMORY;
    let conn_str = format!("file:{unique_id}");
    rusqlite::Connection::open_with_flags(conn_str, flags)
}

#[test]
fn test_create_pool() {
    let new_conn = || new_mem_conn("test_create_pool");
    let pool = ConnectionPool::new(5, new_conn).unwrap();
    assert_eq!(pool.capacity(), 5);
    assert!(pool.all_connections_ready());
}

#[test]
fn test_pop_connection() {
    let new_conn = || new_mem_conn("test_pop_connection");
    let pool = ConnectionPool::new(3, new_conn).unwrap();
    assert!(pool.all_connections_ready());

    let handle = pool.pop().unwrap();
    assert!(!pool.all_connections_ready());

    drop(handle);
    assert!(pool.all_connections_ready());
}

#[test]
fn test_pop_all_connections() {
    let new_conn = || new_mem_conn("test_pop_all_connections");
    let pool = ConnectionPool::new(2, new_conn).unwrap();

    let handle1 = pool.pop().unwrap();
    let handle2 = pool.pop().unwrap();

    assert!(pool.pop().is_none());
    assert!(!pool.all_connections_ready());

    drop(handle1);
    assert!(pool.pop().is_some());

    drop(handle2);
    assert!(pool.all_connections_ready());
}

#[test]
fn test_use_connections() {
    let new_conn = || new_mem_conn("test_use_connections");
    let pool = ConnectionPool::new(3, new_conn).unwrap();

    // Create the table with one handle.
    let handle1 = pool.pop().unwrap();
    handle1
        .execute("CREATE TABLE test (id INTEGER)", ())
        .unwrap();

    // Insert into the table with another handle.
    let handle2 = pool.pop().unwrap();
    handle2
        .execute("INSERT INTO test (id) VALUES (?)", [42])
        .unwrap();

    // Query from another handle.
    let handle3 = pool.pop().unwrap();
    {
        let mut stmt = handle3.prepare("SELECT id FROM test").unwrap();
        let id: i32 = stmt.query_row((), |row| row.get(0)).unwrap();
        assert_eq!(id, 42);
    }

    // No connections left.
    assert!(pool.pop().is_none());

    // Return all connections.
    std::mem::drop((handle1, handle2, handle3));
    assert!(pool.all_connections_ready());
}

#[test]
fn test_close_pool() {
    let new_conn = || new_mem_conn("test_close_pool");
    let pool = ConnectionPool::new(3, new_conn).unwrap();
    let handle = pool.pop().unwrap();
    drop(handle);
    let close_results = pool.close();
    assert_eq!(close_results.len(), 3);
    for result in close_results {
        assert!(result.is_ok());
    }
}
