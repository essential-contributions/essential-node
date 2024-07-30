//! Tests around state.

use essential_node_db as node_db;
use essential_types::{Key, Value};
use rusqlite::Connection;

mod util;

#[test]
fn test_state_value() {
    // The test state.
    let seed = 72;
    let da_block = 10;
    let contract = util::test_contract(seed);
    let key = Key::from([0xAB; 32]);
    let value = Value::from([0xCD; 32]);

    // Create an in-memory SQLite database.
    let mut conn = Connection::open_in_memory().unwrap();
    let tx = conn.transaction().unwrap();
    node_db::create_tables(&tx).unwrap();
    node_db::insert_contract(&tx, &contract, da_block).unwrap();

    // Write some state.
    let contract_ca = essential_hash::contract_addr::from_contract(&contract);
    node_db::update_state(&tx, &contract_ca, &key, &value).unwrap();
    tx.commit().unwrap();

    // Fetch the state value.
    let fetched_value = node_db::get_state_value(&conn, &contract_ca, &key)
        .unwrap()
        .unwrap();

    assert_eq!(value, fetched_value);
}

#[test]
fn test_state_values_with_deletion() {
    // The test state.
    let seed = 36;
    let da_block = 100;
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

    // Create an in-memory SQLite database.
    let mut conn = Connection::open_in_memory().unwrap();
    let tx = conn.transaction().unwrap();

    // Create tables, contract, insert values.
    node_db::create_tables(&tx).unwrap();
    node_db::insert_contract(&tx, &contract, da_block).unwrap();
    let contract_ca = essential_hash::contract_addr::from_contract(&contract);
    for (k, v) in keys.iter().zip(&values) {
        node_db::update_state(&tx, &contract_ca, k, v).unwrap();
    }
    tx.commit().unwrap();

    // Fetch the state values.
    let mut fetched = vec![];
    for k in &keys {
        fetched.push(
            node_db::get_state_value(&conn, &contract_ca, k)
                .unwrap()
                .unwrap(),
        );
    }

    assert_eq!(values, fetched);

    // Delete all state.
    for k in &keys {
        node_db::delete_state(&conn, &contract_ca, k).unwrap();
    }

    // Attempt to fetch the values again.
    for k in &keys {
        let opt = node_db::get_state_value(&conn, &contract_ca, k).unwrap();
        assert!(opt.is_none());
    }
}
