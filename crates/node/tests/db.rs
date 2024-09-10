#![cfg(feature = "test-utils")]

use essential_node::{
    test_utils::{self, test_db_conf},
    Node,
};
use std::sync::Arc;

#[tokio::test]
async fn test_acquire() {
    let conf = test_db_conf();
    let node = Node::new(&conf).unwrap();
    node.db().acquire().await.unwrap();
}

#[test]
fn test_try_acquire() {
    let conf = test_db_conf();
    let node = Node::new(&conf).unwrap();
    node.db().try_acquire().unwrap();
}

#[tokio::test]
async fn test_create_tables() {
    // Tables created during node initialisation.
    let conf = test_db_conf();
    let node = Node::new(&conf).unwrap();

    // Verify that each table exists by querying the SQLite master table
    {
        let conn = node.db().acquire().await.unwrap();
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

    node.close().unwrap();
}

#[tokio::test]
async fn test_block() {
    let conf = test_db_conf();
    let node = Node::new(&conf).unwrap();

    // The test blocks.
    let (blocks, _) = test_utils::test_blocks(100);

    // Insert the blocks.
    let db = node.db();
    for block in &blocks {
        let block = Arc::new(block.clone());
        db.insert_block(block).await.unwrap();
    }

    // Get the blocks.
    let fetched = db.list_blocks(0..blocks.len() as _).await.unwrap();
    assert_eq!(blocks, fetched);

    node.close().unwrap();
}

#[tokio::test]
async fn test_contract() {
    let conf = test_db_conf();
    let node = Node::new(&conf).unwrap();

    // The test contract.
    let seed = 42;
    let da_block = 100;
    let contract = Arc::new(test_utils::test_contract(seed));

    // Insert the contract.
    let clone = contract.clone();
    node.db().insert_contract(clone, da_block).await.unwrap();

    // Get the contract.
    let ca = essential_hash::content_addr(contract.as_ref());
    let fetched = node.db().get_contract(ca).await.unwrap().unwrap();
    assert_eq!(&*contract, &fetched);

    node.close().unwrap();
}

#[tokio::test]
async fn test_state() {
    let conf = test_db_conf();
    let node = Node::new(&conf).unwrap();

    // The test state.
    let seed = 36;
    let da_block = 100;
    let contract = Arc::new(test_utils::test_contract(seed));

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
    node.db()
        .insert_contract(contract.clone(), da_block)
        .await
        .unwrap();
    let contract_ca = essential_hash::content_addr(contract.as_ref());

    // Spawn a task for every insertion.
    let mut handles = vec![];
    for (k, v) in keys.iter().zip(&values) {
        let db = node.db();
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
        let db = node.db();
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
        node.db()
            .delete_state(contract_ca.clone(), k.clone())
            .await
            .unwrap();
    }

    // Attempt to fetch the values again.
    for k in &keys {
        let opt = node
            .db()
            .query_state(contract_ca.clone(), k.clone())
            .await
            .unwrap();
        assert!(opt.is_none());
    }

    node.close().unwrap();
}
