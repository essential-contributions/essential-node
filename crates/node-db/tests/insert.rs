//! Basic tests for testing insertion behaviour.

use essential_hash::content_addr;
use essential_node_db as node_db;
use essential_types::predicate::Predicate;
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
        let mut rows = stmt.query(&[&ca_blob]).unwrap();
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
        .prepare("SELECT content_hash, salt, da_block_number FROM contract")
        .unwrap();
    let (contract_ca_blob, salt_blob, da_block_number) = stmt
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
    assert_eq!(block_n, da_block_number);

    // Check the salt.
    let salt: [u8; 32] = node_db::decode(&salt_blob).unwrap();
    assert_eq!(contract.salt, salt);

    // Verify the predicates were inserted correctly.
    let mut stmt = conn
        .prepare("SELECT predicate FROM predicate ORDER BY id")
        .unwrap();
    let rows = stmt
        .query_map((), |row| Ok(row.get::<_, Vec<u8>>(0)?))
        .unwrap();
    for (row, expected_pred) in rows.into_iter().zip(&contract.predicates) {
        let pred_blob = row.unwrap();
        let pred: Predicate = node_db::decode(&pred_blob).unwrap();
        assert_eq!(&pred, expected_pred);
    }
}
