//! Basic tests for testing insertion behaviour.

use essential_hash::content_addr;
use essential_node_db as node_db;
use essential_types::{predicate::Predicate, ContentAddress};
use rusqlite::Connection;
use std::time::Duration;

mod util;

#[test]
fn test_insert_block() {
    // Test block that we'll insert.
    let block = util::test_block(1, Duration::from_secs(1));

    // Create an in-memory SQLite database
    let mut conn = Connection::open_in_memory().unwrap();

    // Create the necessary tables and insert the block.
    let tx = conn.transaction().unwrap();
    node_db::create_tables(&tx).unwrap();
    node_db::insert_block(&tx, &block).unwrap();
    tx.commit().unwrap();

    // Verify that the block was inserted correctly
    let query = "SELECT number, timestamp_secs, timestamp_nanos FROM block WHERE number = 1";
    let mut stmt = conn.prepare(query).unwrap();
    let mut rows = stmt.query(()).unwrap();

    let row = rows.next().unwrap().unwrap();
    let id: i64 = row.get(0).expect("number");
    let timestamp_secs: u64 = row.get(1).expect("timestamp_secs");
    let timestamp_nanos: u32 = row.get(2).expect("timestamp_nanos");
    let timestamp = Duration::new(timestamp_secs, timestamp_nanos);

    assert_eq!(id, block.number as i64);
    assert_eq!(timestamp, block.timestamp);

    // Verify that the solutions were inserted correctly
    for solution in &block.solutions {
        let ca_blob = node_db::encode(&content_addr(solution));
        let solution_blob = node_db::encode(solution);
        let query = "SELECT solution FROM solution WHERE content_hash = ?";
        let mut stmt = conn.prepare(query).unwrap();
        let mut rows = stmt.query([&ca_blob]).unwrap();
        let row = rows.next().unwrap().unwrap();
        let solution_data: Vec<u8> = row.get(0).unwrap();
        assert_eq!(solution_data, solution_blob);
    }
}

#[test]
fn test_insert_contract() {
    // The test contract that we'll insert.
    let seed = 42;
    let contract = util::test_contract(seed);
    let block_n = 69;

    // Create an in-memory SQLite database.
    let mut conn = Connection::open_in_memory().unwrap();

    // Create the necessary tables and insert the contract.
    let tx = conn.transaction().unwrap();
    node_db::create_tables(&tx).unwrap();
    node_db::insert_contract(&tx, &contract, block_n).expect("Failed to insert contract");
    tx.commit().unwrap();

    // Verify the contract was inserted correctly.
    let mut stmt = conn
        .prepare("SELECT content_hash, salt, l2_block_number FROM contract")
        .unwrap();
    let (contract_ca_blob, salt_blob, l2_block_number) = stmt
        .query_row((), |row| {
            Ok((
                row.get::<_, Vec<u8>>(0)?,
                row.get::<_, Vec<u8>>(1)?,
                row.get::<_, u64>(2)?,
            ))
        })
        .unwrap();
    let expected_contract_ca = essential_hash::contract_addr::from_contract(&contract);
    assert_eq!(
        expected_contract_ca,
        node_db::decode(&contract_ca_blob).unwrap()
    );

    // Check the block number.
    assert_eq!(block_n, l2_block_number);

    // Check the salt.
    let salt: [u8; 32] = node_db::decode(&salt_blob).unwrap();
    assert_eq!(contract.salt, salt);

    // Verify the predicates were inserted correctly.
    let mut stmt = conn
        .prepare("SELECT predicate FROM predicate ORDER BY id")
        .unwrap();
    let rows = stmt.query_map((), |row| row.get::<_, Vec<u8>>(0)).unwrap();
    for (row, expected_pred) in rows.into_iter().zip(&contract.predicates) {
        let pred_blob = row.unwrap();
        let pred: Predicate = node_db::decode(&pred_blob).unwrap();
        assert_eq!(&pred, expected_pred);
    }
}

#[test]
fn test_insert_contract_progress() {
    let mut conn = Connection::open_in_memory().unwrap();
    let tx = conn.transaction().unwrap();
    node_db::create_tables(&tx).unwrap();
    node_db::insert_contract_progress(&tx, 0, &ContentAddress([0; 32]))
        .expect("Failed to insert contract progress");
    tx.commit().unwrap();

    let mut stmt = conn
        .prepare("SELECT id, l2_block_number, content_hash FROM contract_progress")
        .unwrap();
    let mut result = stmt
        .query_map((), |row| {
            Ok((
                row.get::<_, u64>("id")?,
                row.get::<_, u64>("l2_block_number")?,
                row.get::<_, Vec<u8>>("content_hash")?,
            ))
        })
        .unwrap();
    let (id, l2_block_number, content_hash) = result.next().unwrap().unwrap();
    assert_eq!(id, 1);
    assert_eq!(l2_block_number, 0);
    assert_eq!(
        node_db::decode::<ContentAddress>(&content_hash).unwrap(),
        ContentAddress([0; 32])
    );
    assert!(result.next().is_none());

    node_db::insert_contract_progress(&conn, u64::MAX, &ContentAddress([1; 32]))
        .expect("Failed to insert contract progress");

    drop(result);

    let result = node_db::get_contract_progress(&conn).unwrap().unwrap();
    assert_eq!(result.0, u64::MAX);
    assert_eq!(result.1, ContentAddress([1; 32]));

    // Id should always be 1 because we only inserted one row.
    let mut result = stmt.query_map((), |row| row.get::<_, u64>("id")).unwrap();
    let id = result.next().unwrap().unwrap();
    assert_eq!(id, 1);
    drop(result);

    // Check the db only has one row.
    let num_rows = conn
        .query_row("SELECT COUNT(id) FROM contract_progress", (), |row| {
            row.get::<_, i64>("COUNT(id)")
        })
        .unwrap();
    assert_eq!(num_rows, 1);
}

#[test]
fn test_update_state_progress() {
    let mut conn = Connection::open_in_memory().unwrap();
    let tx = conn.transaction().unwrap();
    node_db::create_tables(&tx).unwrap();
    node_db::update_state_progress(&tx, 0, &ContentAddress([0; 32]))
        .expect("Failed to insert state progress");
    tx.commit().unwrap();

    let mut stmt = conn
        .prepare("SELECT id, number, block_hash FROM state_progress WHERE id = 1")
        .unwrap();
    let mut result = stmt
        .query_map((), |row| {
            Ok((
                row.get::<_, u64>("id")?,
                row.get::<_, u64>("number")?,
                row.get::<_, Vec<u8>>("block_hash")?,
            ))
        })
        .unwrap();
    let (id, block_number, block_hash) = result.next().unwrap().unwrap();
    assert_eq!(id, 1);
    assert_eq!(block_number, 0);
    assert_eq!(
        node_db::decode::<ContentAddress>(&block_hash).unwrap(),
        ContentAddress([0; 32])
    );
    assert!(result.next().is_none());

    node_db::update_state_progress(&conn, u64::MAX, &ContentAddress([1; 32]))
        .expect("Failed to insert state progress");

    drop(result);

    let result = node_db::get_state_progress(&conn).unwrap().unwrap();
    assert_eq!(result.0, u64::MAX);
    assert_eq!(result.1, ContentAddress([1; 32]));

    // Id should always be 1 because we only inserted one row.
    let mut result = stmt.query_map((), |row| row.get::<_, u64>("id")).unwrap();
    let id = result.next().unwrap().unwrap();
    assert_eq!(id, 1);
    drop(result);

    // Check the db only has one row.
    let num_rows = conn
        .query_row("SELECT COUNT(id) FROM state_progress", (), |row| {
            row.get::<_, i64>("COUNT(id)")
        })
        .unwrap();
    assert_eq!(num_rows, 1);
}
