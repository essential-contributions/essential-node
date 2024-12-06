//! Basic tests for testing insertion behaviour.

use essential_hash::content_addr;
use essential_node_db::{self as node_db, words_from_blob};
use essential_types::{ContentAddress, Hash, Key, PredicateAddress, Value, Word};
use rusqlite::params;
use std::time::Duration;
use util::{test_blocks, test_blocks_with_vars, test_conn};

mod util;

#[test]
fn test_insert_block() {
    // Test block that we'll insert.
    let (_contract_addr, blocks) = test_blocks_with_vars(10);

    // Create an in-memory SQLite database
    let mut conn = test_conn();

    // Create the necessary tables and insert the block.
    let tx = conn.transaction().unwrap();
    node_db::create_tables(&tx).unwrap();
    for block in &blocks {
        node_db::insert_block(&tx, block).unwrap();
    }
    tx.commit().unwrap();

    for (block_ix, block) in blocks.iter().enumerate() {
        // Verify that the blocks were inserted correctly
        let query = "SELECT number, timestamp_secs, timestamp_nanos FROM block WHERE number = ?";
        let mut stmt = conn.prepare(query).unwrap();
        let mut rows = stmt.query(params![block_ix]).unwrap();

        let row = rows.next().unwrap().unwrap();
        let id: Word = row.get(0).expect("number");
        let timestamp_secs: u64 = row.get(1).expect("timestamp_secs");
        let timestamp_nanos: u32 = row.get(2).expect("timestamp_nanos");
        let timestamp = Duration::new(timestamp_secs, timestamp_nanos);

        assert_eq!(id, block.header.number);
        assert_eq!(timestamp, block.header.timestamp);

        // Verify that the solution sets were inserted correctly
        for solution_set in block.solution_sets.iter() {
            // Verify solution set was inserted
            let solution_set_address = &content_addr(solution_set);
            let query = "SELECT 1 FROM solution_set WHERE content_addr = ?";
            let mut stmt = conn.prepare(query).unwrap();
            let mut rows = stmt.query([&solution_set_address.0]).unwrap();
            let row = rows.next().unwrap().unwrap();
            let solution: Option<bool> = row.get(0).unwrap();
            assert!(solution.is_some());

            for (solution_ix, solution) in solution_set.solutions.iter().enumerate() {
                // Verify solution was inserted corectly
                let query = "SELECT solution.contract_addr, solution.predicate_addr
                    FROM solution JOIN solution_set ON solution.solution_set_id = solution_set.id
                    WHERE solution_set.content_addr = ? AND solution.solution_index = ?";
                let mut stmt = conn.prepare(query).unwrap();
                let mut rows = stmt
                    .query(params![solution_set_address.0, solution_ix as i64])
                    .unwrap();
                let row = rows.next().unwrap().unwrap();
                let contract_addr: Hash = row.get(0).unwrap();
                let predicate_addr: Hash = row.get(1).unwrap();
                assert_eq!(
                    PredicateAddress {
                        contract: ContentAddress(contract_addr),
                        predicate: ContentAddress(predicate_addr)
                    },
                    solution.predicate_to_solve
                );

                // Verify contract to mutation mappings were inserted correctly
                for (mutation_ix, mutation) in solution.state_mutations.iter().enumerate() {
                    // Query deployed contract
                    let query = "SELECT mutation.key FROM mutation
                    JOIN solution ON solution.id = mutation.solution_id
                    JOIN solution_set ON solution.solution_set_id = solution_set.id
                    WHERE solution_set.content_addr = ? AND mutation.mutation_index = ? AND solution.solution_index = ?
                    ORDER BY mutation.mutation_index ASC";
                    let mut stmt = conn.prepare(query).unwrap();
                    let mut result = stmt
                        .query_map(
                            params![
                                solution_set_address.0,
                                mutation_ix as i64,
                                solution_ix as i64
                            ],
                            |row| row.get::<_, Vec<u8>>("key"),
                        )
                        .unwrap();
                    let key_blob = result.next().unwrap().unwrap();
                    let key: Key = words_from_blob(&key_blob);
                    assert_eq!(mutation.key, key);
                }

                // Verify predicate data were inserted correctly
                for (pdi, pred_data) in solution.predicate_data.iter().enumerate() {
                    // Query predicate data
                    let query = "SELECT pred_data.value FROM pred_data
                        JOIN solution ON solution.id = pred_data.solution_id
                        JOIN solution_set ON solution.solution_set_id = solution_set.id
                        WHERE solution.solution_index = ? AND pred_data.pred_data_index = ? AND solution_set.content_addr = ?";
                    let mut stmt = conn.prepare(query).unwrap();
                    let mut pred_data_result = stmt
                        .query_map(params![solution_ix, pdi, solution_set_address.0], |row| {
                            row.get::<_, Vec<u8>>("value")
                        })
                        .unwrap();

                    let value_blob = pred_data_result.next().unwrap().unwrap();
                    let value: Value = words_from_blob(&value_blob);
                    assert_eq!(value, *pred_data);
                }
            }
        }
    }
}

#[test]
fn test_finalize_block() {
    // Number of blocks to insert.
    const NUM_BLOCKS: Word = 3;

    // Number of blocks to finalize.
    const NUM_FINALIZED_BLOCKS: Word = 2;

    if NUM_FINALIZED_BLOCKS > NUM_BLOCKS {
        panic!("NUM_FINALIZED_BLOCKS must be less than or equal to NUM_BLOCKS");
    }

    // Test blocks that we'll insert.
    let blocks = util::test_blocks(NUM_BLOCKS);

    // Create an in-memory SQLite database
    let mut conn = test_conn();

    let r = node_db::with_tx(&mut conn, |tx| {
        // Create the necessary tables and insert blocks
        node_db::create_tables(tx).unwrap();
        for block in &blocks {
            node_db::insert_block(tx, block).unwrap();
        }
        let r = node_db::list_blocks(tx, 0..(NUM_BLOCKS + 10)).unwrap();
        assert_eq!(r.len(), NUM_BLOCKS as usize);
        for (block, expected_block) in blocks.iter().zip(&r) {
            assert_eq!(block, expected_block);
        }

        // Finalize the blocks.
        for block in blocks.iter().take(NUM_FINALIZED_BLOCKS as usize) {
            let block_address = content_addr(block);
            node_db::finalize_block(tx, &block_address).unwrap();
        }

        // Should not change list blocks
        node_db::list_blocks(tx, 0..(NUM_BLOCKS + 10))
    })
    .unwrap();

    assert_eq!(r.len(), NUM_BLOCKS as usize);

    // Check the latest finalized block hash.
    let latest_finalized_block_address =
        node_db::get_latest_finalized_block_address(&conn).unwrap();
    let expected_latest_finalized_block_address =
        content_addr(&blocks[NUM_FINALIZED_BLOCKS as usize - 1]);
    assert_eq!(
        latest_finalized_block_address,
        Some(expected_latest_finalized_block_address)
    );

    let query = "SELECT DISTINCT b.block_address FROM block AS b JOIN finalized_block AS f ON f.block_id = b.id ORDER BY b.number ASC";
    let mut stmt = conn.prepare(query).unwrap();
    let rows: Vec<essential_types::Hash> = stmt
        .query_map([], |row| row.get("block_address"))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(rows.len(), NUM_FINALIZED_BLOCKS as usize);
    rows.iter()
        .zip(blocks.iter())
        .for_each(|(block_address, block)| {
            let expected_block_address = content_addr(block);
            assert_eq!(*block_address, expected_block_address.0);
        });

    drop(stmt);

    // Check that I can't finalize two blocks with the same number
    let fork = util::test_block(
        NUM_FINALIZED_BLOCKS - 1,
        Duration::from_secs((NUM_FINALIZED_BLOCKS + 1000) as u64),
    );

    node_db::with_tx(&mut conn, |tx| node_db::insert_block(tx, &fork)).unwrap();

    let e = node_db::finalize_block(&conn, &content_addr(&fork)).unwrap_err();
    assert!(matches!(
        e,
        rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error {
                code: rusqlite::ErrorCode::ConstraintViolation,
                ..
            },
            _
        )
    ));
}

#[test]
fn test_failed_block() {
    const NUM_BLOCKS: Word = 2;

    // Test blocks that we'll insert.
    let blocks = util::test_blocks(NUM_BLOCKS);

    // Create an in-memory SQLite database
    let mut conn = test_conn();

    // Create the necessary tables and insert the block.
    let r = node_db::with_tx(&mut conn, |tx| {
        node_db::create_tables(tx).unwrap();
        for block in &blocks {
            node_db::insert_block(tx, block).unwrap();
        }
        node_db::list_blocks(tx, 0..(NUM_BLOCKS + 10))
    })
    .unwrap();

    assert_eq!(r.len(), 2);
    assert_eq!(&blocks[0], &r[0]);
    assert_eq!(&blocks[1], &r[1]);

    // Insert failed block.
    let block_address = content_addr(&blocks[0]);
    let solution_set_addr = content_addr(blocks[0].solution_sets.first().unwrap());
    node_db::insert_failed_block(&conn, &block_address, &solution_set_addr).unwrap();

    // Check failed blocks.
    let failed_blocks = node_db::list_failed_blocks(&conn, 0..(NUM_BLOCKS + 10)).unwrap();
    assert_eq!(failed_blocks.len(), 1);
    assert_eq!(failed_blocks[0].0, blocks[0].header.number);
    assert_eq!(failed_blocks[0].1, solution_set_addr);

    // Same failed block should not be inserted again.
    node_db::insert_failed_block(&conn, &block_address, &solution_set_addr).unwrap();
    let failed_blocks = node_db::list_failed_blocks(&conn, 0..(NUM_BLOCKS + 10)).unwrap();
    assert_eq!(failed_blocks.len(), 1);
    assert_eq!(failed_blocks[0].0, blocks[0].header.number);
    assert_eq!(failed_blocks[0].1, solution_set_addr);

    // Insert another failed block.
    let block_address = content_addr(&blocks[1]);
    let solution_set_addr = content_addr(blocks[1].solution_sets.first().unwrap());
    node_db::insert_failed_block(&conn, &block_address, &solution_set_addr).unwrap();

    let failed_blocks = node_db::with_tx_dropped(&mut conn, |tx| {
        let r = node_db::list_blocks(tx, 0..(NUM_BLOCKS + 10)).unwrap();
        assert_eq!(r.len(), 2);
        assert_eq!(&blocks[1], &r[1]);
        // Check failed blocks.
        node_db::list_failed_blocks(tx, 0..(NUM_BLOCKS + 10))
    })
    .unwrap();

    assert_eq!(failed_blocks.len(), 2);
    assert_eq!(failed_blocks[1].0, blocks[1].header.number);
    assert_eq!(failed_blocks[1].1, solution_set_addr);
}

#[test]
fn test_fork_block() {
    let first = util::test_block(0, Duration::from_secs(1));
    let fork_a = util::test_block(1, Duration::from_secs(1));
    let fork_b = util::test_block(1, Duration::from_secs(2));

    // Create an in-memory SQLite database
    let mut conn = test_conn();

    let r = node_db::with_tx(&mut conn, |tx| {
        // Create the necessary tables and insert the block.
        node_db::create_tables(tx).unwrap();
        node_db::insert_block(tx, &first).unwrap();
        node_db::insert_block(tx, &fork_a).unwrap();
        node_db::insert_block(tx, &fork_b).unwrap();

        node_db::list_blocks(tx, 0..10)
    })
    .unwrap();

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
fn test_update_validation_progress() {
    let blocks = test_blocks(2);
    let block_addresses = blocks.iter().map(content_addr).collect::<Vec<_>>();

    let mut conn = test_conn();

    let tx = conn.transaction().unwrap();
    node_db::create_tables(&tx).unwrap();
    for block in &blocks {
        node_db::insert_block(&tx, block).unwrap();
    }
    node_db::update_validation_progress(&tx, &block_addresses[0])
        .expect("Failed to insert validation progress");
    tx.commit().unwrap();

    let mut stmt = conn
        .prepare("SELECT validation_progress.id, block.block_address FROM validation_progress JOIN block ON block.id = validation_progress.block_id")
        .unwrap();
    let mut result = stmt
        .query_map((), |row| {
            Ok((
                row.get::<_, u64>("id")?,
                row.get::<_, Vec<u8>>("block_address")?,
            ))
        })
        .unwrap();
    let (id, block_address) = result.next().unwrap().unwrap();
    assert_eq!(id, 1);
    assert_eq!(block_address, block_addresses[0].0);
    assert!(result.next().is_none());

    node_db::update_validation_progress(&conn, &block_addresses[1])
        .expect("Failed to insert validation progress");

    drop(result);

    let result = node_db::get_validation_progress(&conn).unwrap().unwrap();
    assert_eq!(result, block_addresses[1]);

    // Id should always be 1 because we only inserted one row.
    let mut result = stmt.query_map((), |row| row.get::<_, u64>("id")).unwrap();
    let id = result.next().unwrap().unwrap();
    assert_eq!(id, 1);
    drop(result);

    // Check the db only has one row.
    let num_rows = conn
        .query_row("SELECT COUNT(id) FROM validation_progress", (), |row| {
            row.get::<_, i64>("COUNT(id)")
        })
        .unwrap();
    assert_eq!(num_rows, 1);
}
