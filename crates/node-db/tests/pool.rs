use essential_node_db::{self as db, ConnectionPool};
use std::{sync::Arc, time::Duration};
use tempfile::TempDir;
use util::{register_contracts_block, test_conn_pool, test_contract_registry};

mod util;

#[test]
fn test_conn_pool_new() {
    let _db = test_conn_pool();
}

#[test]
fn test_conn_pool_close() {
    let db = test_conn_pool();
    db.close().unwrap();
}

#[tokio::test]
async fn test_acquire() {
    let db = test_conn_pool();
    db.acquire().await.unwrap();
}

#[test]
fn test_try_acquire() {
    let db = test_conn_pool();
    db.try_acquire().unwrap();
}

#[tokio::test]
async fn test_conn_pool_path() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("test_conn_pool_path.sqlite3");
    let conf = db::pool::Config {
        source: db::pool::Source::Path(path.clone()),
        ..Default::default()
    };
    let db = ConnectionPool::with_tables(&conf).unwrap();
    let conn = db.acquire().await.unwrap();
    conn.pragma_query(None, "trusted_schema", |row| {
        let val = row.get::<_, bool>(0)?;
        assert!(!val);
        Ok(())
    })
    .unwrap();
    conn.pragma_query(None, "foreign_keys", |row| {
        let val = row.get::<_, bool>(0)?;
        assert!(val);
        Ok(())
    })
    .unwrap();
    conn.pragma_query(None, "synchronous", |row| {
        let val = row.get::<_, i64>(0)?;
        assert_eq!(val, 1);
        Ok(())
    })
    .unwrap();

    // Reopen the database for the `journal_mode` change to be effective.
    drop(db);
    let db = ConnectionPool::with_tables(&conf).unwrap();
    let conn = db.acquire().await.unwrap();

    conn.pragma_query(None, "journal_mode", |row| {
        let val = row.get::<_, String>(0)?;
        assert_eq!(val, "wal");
        Ok(())
    })
    .unwrap();
}

#[tokio::test]
async fn test_create_tables() {
    // Tables created during node initialisation.
    let db = test_conn_pool();

    // Verify that each table exists by querying the SQLite master table
    {
        let conn = db.acquire().await.unwrap();
        for table in essential_node_db::sql::table::ALL {
            let query = format!(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='{}';",
                table.name,
            );
            let result: String = conn
                .query_row(&query, (), |row| row.get(0))
                .unwrap_or_else(|_| panic!("Table {} does not exist", table.name));
            assert_eq!(
                result, table.name,
                "Table {} was not created successfully",
                table.name,
            );
        }
    }

    db.close().unwrap();
}

#[tokio::test]
async fn test_block() {
    let db = test_conn_pool();

    // The test blocks.
    let blocks = util::test_blocks(100);

    // Insert the blocks.
    for block in &blocks {
        let block = Arc::new(block.clone());
        db.insert_block(block).await.unwrap();
    }

    // Get the blocks.
    let fetched = db.list_blocks(0..blocks.len() as _).await.unwrap();
    assert_eq!(blocks, fetched);

    db.close().unwrap();
}

#[tokio::test]
async fn test_contract() {
    let db = test_conn_pool();

    // The test contract.
    let seed = 42;
    let contract = util::test_contract(seed);

    // Insert the contract.
    let registry = test_contract_registry();
    let block =
        register_contracts_block(registry, Some(&contract), 42, Duration::from_secs(42)).unwrap();
    db.insert_block(block.into()).await.unwrap();

    // Get the contract.
    let _ca = essential_hash::content_addr(&contract);

    // TODO: Re-add this upon deciding how to retrieve contracts from DB.
    // let fetched = db.get_contract(ca).await.unwrap().unwrap();
    // assert_eq!(&*contract, &fetched);

    db.close().unwrap();
}

#[tokio::test]
async fn test_state() {
    let db = test_conn_pool();

    // The test state.
    let seed = 36;
    let number = 100;
    let contract = util::test_contract(seed);

    // Make some randomish keys and values.
    let mut keys = vec![];
    let mut values = vec![];
    for i in 0i64..1024 {
        let key = vec![(i + 1) * 5; ((i + 1) as usize * 103) % 128];
        let value = vec![(i + 1) * 7; ((i + 1) as usize * 391) % 128];
        keys.push(key);
        values.push(value);
    }

    // Insert a contract to own the state.
    let registry = test_contract_registry();
    let timestamp = Duration::from_secs(42);
    let block = register_contracts_block(registry, Some(&contract), number, timestamp).unwrap();
    db.insert_block(block.into()).await.unwrap();
    let contract_ca = essential_hash::content_addr(&contract);

    // Spawn a task for every insertion.
    let mut handles = vec![];
    for (k, v) in keys.iter().zip(&values) {
        let db = db.clone();
        let ca = contract_ca.clone();
        let (key, value) = (k.clone(), v.clone());
        let handle = tokio::spawn(async move { db.update_state(ca, key, value).await });
        handles.push(handle);
    }

    // Wait for the insertions to complete.
    for handle in handles {
        handle.await.unwrap().unwrap();
    }

    // Fetch the state values concurrently.
    let mut handles = vec![];
    for k in keys.iter() {
        let db = db.clone();
        let ca = contract_ca.clone();
        let key = k.clone();
        handles.push(tokio::spawn(async move { db.query_state(ca, key).await }));
    }

    // Collect the results.
    let mut fetched = vec![];
    for handle in handles {
        let value = handle.await.unwrap().unwrap().unwrap();
        fetched.push(value);
    }

    assert_eq!(values, fetched);

    // Delete all state.
    for k in &keys {
        db.delete_state(contract_ca.clone(), k.clone())
            .await
            .unwrap();
    }

    // Attempt to fetch the values again.
    for k in &keys {
        let opt = db
            .query_state(contract_ca.clone(), k.clone())
            .await
            .unwrap();
        assert!(opt.is_none());
    }

    db.close().unwrap();
}
