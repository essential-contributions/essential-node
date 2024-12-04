use essential_hash::content_addr;
use essential_node_db::{self as node_db};
use essential_types::{Block, ContentAddress, Word};
use std::time::Duration;
use util::{test_block, test_blocks_with_vars, test_conn};

mod util;

#[test]
fn test_get_solution_set() {
    // The test block.
    let block = util::test_block(42, Duration::from_secs(69));

    // Create an in-memory SQLite database.
    let mut conn = test_conn();

    // Create the necessary tables and insert the block.
    let tx = conn.transaction().unwrap();
    node_db::create_tables(&tx).unwrap();
    node_db::insert_block(&tx, &block).unwrap();

    // Fetch the solution sets.
    for solution_set in &block.solution_sets {
        let sol_set_ca = essential_hash::content_addr(solution_set);
        let fetched_solution_set = node_db::get_solution_set(&tx, &sol_set_ca).unwrap();
        assert_eq!(solution_set, &fetched_solution_set);
    }
}

#[test]
fn test_get_repeated_solution_set() {
    // The test solution set and blocks.
    let solution_set = util::test_solution_set(42);
    let block = Block {
        number: 1,
        timestamp: Duration::from_secs(1),
        solution_sets: vec![solution_set.clone()],
    };
    let block2 = Block {
        number: 2,
        timestamp: Duration::from_secs(2),
        solution_sets: vec![solution_set.clone()],
    };

    // Create an in-memory SQLite database.
    let mut conn = test_conn();

    // Create the necessary tables and insert the blocks.
    let tx = conn.transaction().unwrap();
    node_db::create_tables(&tx).unwrap();
    node_db::insert_block(&tx, &block).unwrap();
    node_db::insert_block(&tx, &block2).unwrap();

    // Fetch the first solution set.
    let fetched_solution_set =
        node_db::get_solution_set(&tx, &content_addr(&solution_set)).unwrap();
    let fetched_block = node_db::get_block(&tx, &content_addr(&block))
        .unwrap()
        .unwrap();
    let fetched_block2 = node_db::get_block(&tx, &content_addr(&block2))
        .unwrap()
        .unwrap();

    assert_eq!(solution_set, fetched_solution_set);
    assert_eq!(block, fetched_block);
    assert_eq!(block2, fetched_block2);
}

#[test]
fn test_block_solution_set_ordering() {
    // The test solution set and blocks.
    let solution_set = util::test_solution_set(42);
    let solution_set2 = util::test_solution_set(43);
    let block = Block {
        number: 1,
        timestamp: Duration::from_secs(1),
        solution_sets: vec![solution_set.clone()],
    };
    let block2 = Block {
        number: 2,
        timestamp: Duration::from_secs(2),
        solution_sets: vec![solution_set2.clone(), solution_set.clone()],
    };

    // Create an in-memory SQLite database.
    let mut conn = test_conn();

    // Create the necessary tables and insert the blocks.
    let tx = conn.transaction().unwrap();
    node_db::create_tables(&tx).unwrap();
    node_db::insert_block(&tx, &block).unwrap();
    node_db::insert_block(&tx, &block2).unwrap();

    // Fetch the first solution set.
    let fetched_solution_set =
        node_db::get_solution_set(&tx, &content_addr(&solution_set)).unwrap();
    let fetched_solution_set2 =
        node_db::get_solution_set(&tx, &content_addr(&solution_set2)).unwrap();
    let fetched_block = node_db::get_block(&tx, &content_addr(&block))
        .unwrap()
        .unwrap();
    let fetched_block2 = node_db::get_block(&tx, &content_addr(&block2))
        .unwrap()
        .unwrap();

    assert_eq!(solution_set, fetched_solution_set);
    assert_eq!(solution_set2, fetched_solution_set2);
    assert_eq!(block, fetched_block);
    assert_eq!(block2, fetched_block2);
}

#[test]
fn test_get_validation_progress() {
    // Create test block.
    let block = test_block(42, Default::default());
    let block_address = content_addr(&block);

    // Create an in-memory SQLite database.
    let mut conn = test_conn();

    node_db::with_tx(&mut conn, |tx| {
        // Create the necessary tables and insert the contract progress.
        node_db::create_tables(tx).unwrap();
        node_db::insert_block(tx, &block).unwrap();
        node_db::update_validation_progress(tx, &block_address)
    })
    .unwrap();

    // Fetch the state progress.
    let fetched_block_address = node_db::get_validation_progress(&conn).unwrap().unwrap();

    assert_eq!(fetched_block_address, block_address);
}

#[test]
fn test_list_blocks() {
    // The test blocks.
    let blocks = util::test_blocks(100);

    // Create an in-memory SQLite database.
    let mut conn = test_conn();

    let fetched_blocks = node_db::with_tx(&mut conn, |tx| {
        // Create the necessary tables and insert blocks
        node_db::create_tables(tx).unwrap();
        for block in &blocks {
            node_db::insert_block(tx, block).unwrap();
        }
        node_db::list_blocks(tx, 0..100)
    })
    .unwrap();
    assert_eq!(blocks, fetched_blocks);
}

#[test]
fn test_list_blocks_by_time() {
    // The test blocks.
    let blocks = util::test_blocks(10);

    // Create an in-memory SQLite database.
    let mut conn = test_conn();

    // Create the necessary tables and insert blocks.
    let tx = conn.transaction().unwrap();
    node_db::create_tables(&tx).unwrap();
    for block in &blocks {
        node_db::insert_block(&tx, block).unwrap();
    }

    // List the blocks by time.
    let start_time = Duration::from_secs(3);
    let end_time = Duration::from_secs(6);
    let fetched_blocks = node_db::list_blocks_by_time(&tx, start_time..end_time, 10, 0).unwrap();
    tx.commit().unwrap();

    // Filter the original blocks to match the time range.
    let expected_blocks: Vec<_> = blocks
        .into_iter()
        .filter(|block| block.timestamp >= start_time && block.timestamp < end_time)
        .collect();

    assert_eq!(expected_blocks, fetched_blocks);
}

#[test]
fn get_block_header() {
    let blocks = util::test_blocks(10);
    let headers: Vec<_> = blocks.iter().map(|b| (b.number, b.timestamp)).collect();

    // Create an in-memory SQLite database.
    let mut conn = test_conn();

    // Create the necessary tables and insert blocks.
    let tx = conn.transaction().unwrap();
    node_db::create_tables(&tx).unwrap();
    for block in &blocks {
        node_db::insert_block(&tx, block).unwrap();
    }
    tx.commit().unwrap();

    // Fetch the headers and check they match.
    let fetched_headers: Vec<_> = blocks
        .iter()
        .map(essential_hash::content_addr)
        .map(|ca| node_db::get_block_header(&conn, &ca).unwrap().unwrap())
        .collect();

    assert_eq!(&headers, &fetched_headers);
}

#[test]
fn test_query_at_finalized() {
    // Test block that we'll insert.
    let (contract_addr, blocks) = test_blocks_with_vars(10);

    // Create an in-memory SQLite database.
    let mut conn = test_conn();
    let tx = conn.transaction().unwrap();
    node_db::create_tables(&tx).unwrap();
    for block in &blocks {
        node_db::insert_block(&tx, block).unwrap();
        let block_address = essential_hash::content_addr(block);
        node_db::finalize_block(&tx, &block_address).unwrap();
    }

    // Test queries at each block and solution set.
    for block in &blocks {
        for (ssi, solution_set) in block.solution_sets.iter().enumerate() {
            for (si, solution) in solution_set.solutions.iter().enumerate() {
                for (mi, mutation) in solution.state_mutations.iter().enumerate() {
                    let state = node_db::finalized::query_state_inclusive_block(
                        &tx,
                        &contract_addr,
                        &mutation.key,
                        block.number,
                    )
                    .unwrap()
                    .unwrap();
                    assert_eq!(
                        state, block.solution_sets[2].solutions[si].state_mutations[mi].value,
                        "block: {}, sol_set: {}, sol, {}, mut: {}, k: {:?}, v: {:?}",
                        block.number, ssi, si, mi, mutation.key, mutation.value
                    );

                    let state = node_db::finalized::query_state_exclusive_block(
                        &tx,
                        &contract_addr,
                        &mutation.key,
                        block.number,
                    )
                    .unwrap();

                    if block.number == 0 {
                        assert_eq!(state, None);
                    } else {
                        assert_eq!(
                            state.unwrap(),
                            blocks[(block.number - 1) as usize].solution_sets[2].solutions[si]
                                .state_mutations[mi]
                                .value
                        );
                    }

                    let state = node_db::finalized::query_state_inclusive_solution_set(
                        &tx,
                        &contract_addr,
                        &mutation.key,
                        block.number,
                        ssi as u64,
                    )
                    .unwrap()
                    .unwrap();
                    assert_eq!(
                        state, mutation.value,
                        "block: {}, sol_set: {}, sol, {}, mut: {}, k: {:?}, v: {:?}",
                        block.number, ssi, si, mi, mutation.key, mutation.value
                    );

                    let state = node_db::finalized::query_state_exclusive_solution_set(
                        &tx,
                        &contract_addr,
                        &mutation.key,
                        block.number,
                        ssi as u64,
                    )
                    .unwrap();

                    if block.number == 0 && ssi == 0 {
                        assert_eq!(state, None);
                    } else if ssi == 0 {
                        assert_eq!(
                            state.unwrap(),
                            blocks[(block.number - 1) as usize].solution_sets[2].solutions[si]
                                .state_mutations[mi]
                                .value
                        );
                    } else {
                        assert_eq!(
                            state.unwrap(),
                            block.solution_sets[ssi - 1].solutions[si].state_mutations[mi].value
                        );
                    }
                }
            }
        }
    }

    // Test queries past the end.

    let state = node_db::finalized::query_state_inclusive_block(&tx, &contract_addr, &vec![0], 10)
        .unwrap()
        .unwrap();

    assert_eq!(
        state,
        blocks
            .last()
            .unwrap()
            .solution_sets
            .last()
            .unwrap()
            .solutions[0]
            .state_mutations[0]
            .value
    );

    let state =
        node_db::finalized::query_state_inclusive_solution_set(&tx, &contract_addr, &vec![0], 9, 5)
            .unwrap()
            .unwrap();

    assert_eq!(
        state,
        blocks
            .last()
            .unwrap()
            .solution_sets
            .last()
            .unwrap()
            .solutions[0]
            .state_mutations[0]
            .value
    );

    let state = node_db::finalized::query_state_inclusive_solution_set(
        &tx,
        &contract_addr,
        &vec![0],
        100,
        100,
    )
    .unwrap()
    .unwrap();

    assert_eq!(
        state,
        blocks
            .last()
            .unwrap()
            .solution_sets
            .last()
            .unwrap()
            .solutions[0]
            .state_mutations[0]
            .value
    );

    let state = node_db::finalized::query_state_exclusive_block(&tx, &contract_addr, &vec![0], 10)
        .unwrap()
        .unwrap();

    assert_eq!(
        state,
        blocks
            .last()
            .unwrap()
            .solution_sets
            .last()
            .unwrap()
            .solutions[0]
            .state_mutations[0]
            .value
    );

    let state =
        node_db::finalized::query_state_exclusive_solution_set(&tx, &contract_addr, &vec![0], 9, 5)
            .unwrap()
            .unwrap();

    assert_eq!(
        state,
        blocks
            .last()
            .unwrap()
            .solution_sets
            .last()
            .unwrap()
            .solutions[0]
            .state_mutations[0]
            .value
    );

    let state = node_db::finalized::query_state_exclusive_solution_set(
        &tx,
        &contract_addr,
        &vec![0],
        100,
        100,
    )
    .unwrap()
    .unwrap();

    assert_eq!(
        state,
        blocks
            .last()
            .unwrap()
            .solution_sets
            .last()
            .unwrap()
            .solutions[0]
            .state_mutations[0]
            .value
    );
}

#[test]
fn test_query_state_block_address() {
    // Test block that we'll insert.
    let (contract_addr, blocks) = test_blocks_with_vars(10);

    // Create an in-memory SQLite database.
    let mut conn = test_conn();
    let tx = conn.transaction().unwrap();
    node_db::create_tables(&tx).unwrap();

    let mut prev_addr: Option<ContentAddress> = None;
    for block in &blocks[0..5] {
        let block_address = node_db::insert_block(&tx, block).unwrap();
        node_db::finalize_block(&tx, &block_address).unwrap();
        if let Some(prev_addr) = &prev_addr {
            tx.execute(
                "UPDATE block SET parent_block_id = (SELECT id FROM block WHERE block_address = ?) WHERE block_address = ?",
                [prev_addr.0, block_address.0],
            )
            .unwrap();
        }
        prev_addr = Some(block_address);
    }

    let forks = blocks[5..]
        .iter()
        .chain(blocks[5..].iter())
        .enumerate()
        .map(|(i, block)| {
            let mut fork = block.clone();
            for solution_set in &mut fork.solution_sets {
                for solution in &mut solution_set.solutions {
                    for mutation in &mut solution.state_mutations {
                        mutation.value[0] = (mutation.value[0] * 1000) + (i as Word);
                    }
                }
            }
            fork
        });
    for block in &blocks[5..] {
        let block_address = node_db::insert_block(&tx, block).unwrap();
        if let Some(prev_addr) = &prev_addr {
            tx.execute(
                "UPDATE block SET parent_block_id = (SELECT id FROM block WHERE block_address = ?) WHERE block_address = ?",
                [prev_addr.0, block_address.0],
            )
            .unwrap();
        }
        prev_addr = Some(block_address);
    }

    for fork in forks {
        let block_address = node_db::insert_block(&tx, &fork).unwrap();
        let prev_addr = content_addr(&blocks[fork.number as usize - 1]);
        tx.execute(
            "UPDATE block SET parent_block_id = (SELECT id FROM block WHERE block_address = ?) WHERE block_address = ?",
            [prev_addr.0, block_address.0],
        )
        .unwrap();
    }

    // Test queries at each block and solution set.
    for block in &blocks {
        for (ssi, solution_set) in block.solution_sets.iter().enumerate() {
            for (si, solution) in solution_set.solutions.iter().enumerate() {
                for (mi, mutation) in solution.state_mutations.iter().enumerate() {
                    let state = node_db::address::query_state_inclusive_block(
                        &tx,
                        &contract_addr,
                        &mutation.key,
                        &content_addr(block),
                    )
                    .unwrap()
                    .unwrap();
                    assert_eq!(
                        state, block.solution_sets[2].solutions[si].state_mutations[mi].value,
                        "block: {}, sol_set: {}, sol: {}, mut: {}, k: {:?}, v: {:?}",
                        block.number, ssi, si, mi, mutation.key, mutation.value
                    );

                    let state = node_db::address::query_state_exclusive_block(
                        &tx,
                        &contract_addr,
                        &mutation.key,
                        &content_addr(block),
                    )
                    .unwrap();

                    if block.number == 0 {
                        assert_eq!(state, None);
                    } else {
                        assert_eq!(
                            state.unwrap(),
                            blocks[(block.number - 1) as usize].solution_sets[2].solutions[si]
                                .state_mutations[mi]
                                .value
                        );
                    }

                    let state = node_db::address::query_state_inclusive_solution_set(
                        &tx,
                        &contract_addr,
                        &mutation.key,
                        &content_addr(block),
                        ssi as u64,
                    )
                    .unwrap()
                    .unwrap();
                    assert_eq!(
                        state, mutation.value,
                        "block: {}, sol_set: {}, sol: {}, mut: {}, k: {:?}, v: {:?}",
                        block.number, ssi, si, mi, mutation.key, mutation.value
                    );

                    let state = node_db::address::query_state_exclusive_solution_set(
                        &tx,
                        &contract_addr,
                        &mutation.key,
                        &content_addr(block),
                        ssi as u64,
                    )
                    .unwrap();

                    if block.number == 0 && ssi == 0 {
                        assert_eq!(state, None);
                    } else if ssi == 0 {
                        assert_eq!(
                            state.unwrap(),
                            blocks[(block.number - 1) as usize].solution_sets[2].solutions[si]
                                .state_mutations[mi]
                                .value
                        );
                    } else {
                        assert_eq!(
                            state.unwrap(),
                            block.solution_sets[ssi - 1].solutions[si].state_mutations[mi].value
                        );
                    }
                }
            }
        }
    }

    // Test queries past the end.
    let state = node_db::address::query_state_inclusive_solution_set(
        &tx,
        &contract_addr,
        &vec![0],
        &content_addr(&blocks[9]),
        5,
    )
    .unwrap()
    .unwrap();

    assert_eq!(
        state,
        blocks
            .last()
            .unwrap()
            .solution_sets
            .last()
            .unwrap()
            .solutions[0]
            .state_mutations[0]
            .value
    );

    let state = node_db::address::query_state_exclusive_solution_set(
        &tx,
        &contract_addr,
        &vec![0],
        &content_addr(&blocks[9]),
        5,
    )
    .unwrap()
    .unwrap();

    assert_eq!(
        state,
        blocks
            .last()
            .unwrap()
            .solution_sets
            .last()
            .unwrap()
            .solutions[0]
            .state_mutations[0]
            .value
    );
}
