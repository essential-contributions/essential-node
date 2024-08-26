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
fn test_finalize_block() {
    // Number of blocks to insert.
    const NUM_BLOCKS: u64 = 3;

    // Number of blocks to finalize.
    const NUM_FINALIZED_BLOCKS: u64 = 2;

    if NUM_FINALIZED_BLOCKS > NUM_BLOCKS {
        panic!("NUM_FINALIZED_BLOCKS must be less than or equal to NUM_BLOCKS");
    }

    // Test blocks that we'll insert.
    let blocks = util::test_blocks(NUM_BLOCKS);

    // Create an in-memory SQLite database
    let mut conn = Connection::open_in_memory().unwrap();

    // Create the necessary tables and insert the block.
    let tx = conn.transaction().unwrap();
    node_db::create_tables(&tx).unwrap();
    for block in &blocks {
        node_db::insert_block(&tx, block).unwrap();
    }
    tx.commit().unwrap();

    let r = node_db::list_blocks(&conn, 0..(NUM_BLOCKS + 10)).unwrap();
    assert_eq!(blocks.len(), NUM_BLOCKS as usize);

    for (block, expected_block) in blocks.iter().zip(&r) {
        assert_eq!(block, expected_block);
    }

    // Finalize the blocks.
    let tx = conn.transaction().unwrap();
    for block in blocks.iter().take(NUM_FINALIZED_BLOCKS as usize) {
        let block_hash = content_addr(block);
        node_db::finalize_block(&tx, &block_hash).unwrap();
    }
    tx.commit().unwrap();

    // Should not change list blocks
    let r = node_db::list_blocks(&conn, 0..(NUM_BLOCKS + 10)).unwrap();
    assert_eq!(r.len(), NUM_BLOCKS as usize);

    // Check the latest finalized block hash.
    let latest_finalized_block_hash = node_db::get_latest_finalized_block_hash(&conn).unwrap();
    let expected_latest_finalized_block_hash =
        content_addr(&blocks[NUM_FINALIZED_BLOCKS as usize - 1]);
    assert_eq!(
        latest_finalized_block_hash,
        Some(expected_latest_finalized_block_hash)
    );

    let query = "SELECT DISTINCT b.block_hash FROM block AS b JOIN finalized_block AS f ON f.block_id = b.id ORDER BY b.number ASC";
    let mut stmt = conn.prepare(query).unwrap();
    let rows: Vec<essential_types::Hash> = stmt
        .query_map([], |row| row.get("block_hash"))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(rows.len(), NUM_FINALIZED_BLOCKS as usize);
    rows.iter()
        .zip(blocks.iter())
        .for_each(|(block_hash, block)| {
            let expected_block_hash = content_addr(block);
            assert_eq!(*block_hash, expected_block_hash.0);
        });
}

#[test]
fn test_fork_block() {
    let first = util::test_block(0, Duration::from_secs(1));
    let fork_a = util::test_block(1, Duration::from_secs(1));
    let fork_b = util::test_block(1, Duration::from_secs(2));

    // Create an in-memory SQLite database
    let mut conn = Connection::open_in_memory().unwrap();

    let tx = conn.transaction().unwrap();
    node_db::create_tables(&tx).unwrap();
    node_db::insert_block(&tx, &first).unwrap();
    node_db::insert_block(&tx, &fork_a).unwrap();
    node_db::insert_block(&tx, &fork_b).unwrap();
    tx.commit().unwrap();

    let r = node_db::list_blocks(&conn, 0..10).unwrap();
    assert_eq!(r.len(), 3);
    assert_eq!(r[0], first);

    if r[1] == fork_a {
        assert_eq!(r[2], fork_b);
    } else {
        assert_eq!(r[1], fork_b);
        assert_eq!(r[2], fork_a);
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
