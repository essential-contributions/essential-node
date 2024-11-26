//! Tests around state.

use essential_node_db::{self as node_db, QueryError};
use essential_types::{ContentAddress, Key, Value};
use util::test_conn;

mod util;

#[test]
fn test_state_value() {
    // The test state.
    let seed = 72;
    let contract = util::test_contract(seed);
    let key = Key::from([0xAB; 32]);
    let value = Value::from([0xCD; 32]);

    // Create an in-memory SQLite database.
    let mut conn = test_conn();

    let mut contract_ca = ContentAddress([0u8; 32]);
    node_db::with_tx::<_, QueryError>(&mut conn, |tx| {
        node_db::create_tables(tx)?;
        // Write some state.
        contract_ca = essential_hash::content_addr(&contract);
        node_db::update_state(tx, &contract_ca, &key, &value)?;
        Ok(())
    })
    .unwrap();

    // Fetch the state value.
    let fetched_value = node_db::query_state(&conn, &contract_ca, &key)
        .unwrap()
        .unwrap();

    assert_eq!(value, fetched_value);
}

#[test]
fn test_state_values_with_deletion() {
    // The test state.
    let seed = 36;
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
    let mut conn = test_conn();
    let mut contract_ca = ContentAddress([0u8; 32]);
    node_db::with_tx::<_, QueryError>(&mut conn, |tx| {
        // Create tables, contract, insert values.
        node_db::create_tables(tx)?;
        contract_ca = essential_hash::content_addr(&contract);
        for (k, v) in keys.iter().zip(&values) {
            node_db::update_state(tx, &contract_ca, k, v)?;
        }
        Ok(())
    })
    .unwrap();

    // Fetch the state values.
    let mut fetched = vec![];
    for k in &keys {
        fetched.push(
            node_db::query_state(&conn, &contract_ca, k)
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
        let opt = node_db::query_state(&conn, &contract_ca, k).unwrap();
        assert!(opt.is_none());
    }
}
