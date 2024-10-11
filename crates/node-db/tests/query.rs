use essential_hash::content_addr;
use essential_node_db::{self as node_db};
use essential_types::{contract::Contract, ContentAddress, Word};
use std::time::Duration;
use util::{test_block, test_blocks_with_vars, test_conn};

mod util;

#[test]
fn get_contract_salt() {
    // The test contract.
    let seed = 42;
    let da_block = 100;
    let contract = util::test_contract(seed);

    // Create an in-memory SQLite database.
    let mut conn = test_conn();

    // Create the necessary tables and insert a contract.
    let tx = conn.transaction().unwrap();
    node_db::create_tables(&tx).unwrap();
    node_db::insert_contract(&tx, &contract, da_block).unwrap();
    tx.commit().unwrap();

    // Fetch the salt.
    let ca = essential_hash::content_addr(&contract);
    let salt = node_db::get_contract_salt(&conn, &ca).unwrap().unwrap();

    assert_eq!(contract.salt, salt);
}

#[test]
fn get_contract_predicates() {
    // The test contract.
    let seed = 23;
    let da_block = 100;
    let contract = util::test_contract(seed);

    // Create an in-memory SQLite database.
    let mut conn = test_conn();

    // Create the necessary tables and insert a contract.
    let tx = conn.transaction().unwrap();
    node_db::create_tables(&tx).unwrap();
    node_db::insert_contract(&tx, &contract, da_block).unwrap();
    tx.commit().unwrap();

    // Fetch the predicates.
    let ca = essential_hash::content_addr(&contract);
    let predicates = node_db::get_contract_predicates(&conn, &ca)
        .unwrap()
        .unwrap();

    assert_eq!(contract.predicates, predicates);
}

#[test]
fn test_get_contract() {
    // The test contract.
    let seed = 69;
    let da_block = 100;
    let contract = util::test_contract(seed);

    // Create an in-memory SQLite database.
    let mut conn = test_conn();

    // Create the necessary tables and insert a contract.
    let tx = conn.transaction().unwrap();
    node_db::create_tables(&tx).unwrap();
    node_db::insert_contract(&tx, &contract, da_block).unwrap();
    tx.commit().unwrap();

    // Fetch the contract.
    let ca = essential_hash::content_addr(&contract);
    let fetched_contract = node_db::get_contract(&conn, &ca).unwrap().unwrap();

    assert_eq!(contract, fetched_contract);
}

#[test]
fn test_get_contract_progress() {
    // Create an in-memory SQLite database.
    let mut conn = test_conn();

    // Create the necessary tables and insert the contract progress.
    let tx = conn.transaction().unwrap();
    node_db::create_tables(&tx).unwrap();
    node_db::insert_contract_progress(&tx, 42, &ContentAddress([42; 32])).unwrap();
    tx.commit().unwrap();

    // Fetch the contract progress.
    let (l2_block_number, hash) = node_db::get_contract_progress(&conn).unwrap().unwrap();

    assert_eq!(l2_block_number, 42);
    assert_eq!(hash, ContentAddress([42; 32]));
}

#[test]
fn test_get_predicate() {
    // The test contract.
    let seed = 70;
    let da_block = 100;
    let contract = util::test_contract(seed);

    // Create an in-memory SQLite database.
    let mut conn = test_conn();

    // Create the necessary tables and insert a contract.
    let tx = conn.transaction().unwrap();
    node_db::create_tables(&tx).unwrap();
    node_db::insert_contract(&tx, &contract, da_block).unwrap();
    tx.commit().unwrap();

    // Fetch the first predicate.
    let predicate = &contract.predicates[0];
    let pred_ca = essential_hash::content_addr(predicate);
    let fetched_pred = node_db::get_predicate(&conn, &pred_ca).unwrap().unwrap();

    assert_eq!(predicate, &fetched_pred);
}

#[test]
fn test_get_solution() {
    // The test solution.
    let block = util::test_block(42, Duration::from_secs(69));

    // Create an in-memory SQLite database.
    let mut conn = test_conn();

    // Create the necessary tables and insert the block.
    let tx = conn.transaction().unwrap();
    node_db::create_tables(&tx).unwrap();
    node_db::insert_block(&tx, &block).unwrap();
    tx.commit().unwrap();

    // Fetch the first solution.
    let solution = &block.solutions[0];
    let sol_ca = essential_hash::content_addr(solution);
    let fetched_solution = node_db::get_solution(&conn, &sol_ca).unwrap().unwrap();

    assert_eq!(solution, &fetched_solution);
}

#[test]
fn test_get_state_progress() {
    // Create test block.
    let block = test_block(42, Default::default());
    let block_address = content_addr(&block);

    // Create an in-memory SQLite database.
    let mut conn = test_conn();

    // Create the necessary tables and insert the contract progress.
    let tx = conn.transaction().unwrap();
    node_db::create_tables(&tx).unwrap();
    node_db::insert_block(&tx, &block).unwrap();
    node_db::update_state_progress(&tx, &block_address).unwrap();
    tx.commit().unwrap();

    // Fetch the state progress.
    let fetched_block_address = node_db::get_state_progress(&conn).unwrap().unwrap();

    assert_eq!(fetched_block_address, block_address);
}

#[test]
fn test_get_validation_progress() {
    // Create test block.
    let block = test_block(42, Default::default());
    let block_address = content_addr(&block);

    // Create an in-memory SQLite database.
    let mut conn = test_conn();

    // Create the necessary tables and insert the contract progress.
    let tx = conn.transaction().unwrap();
    node_db::create_tables(&tx).unwrap();
    node_db::insert_block(&tx, &block).unwrap();
    node_db::update_validation_progress(&tx, &block_address).unwrap();
    tx.commit().unwrap();

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

    // Create the necessary tables and insert blocks.
    let tx = conn.transaction().unwrap();
    node_db::create_tables(&tx).unwrap();
    for block in &blocks {
        node_db::insert_block(&tx, block).unwrap();
    }
    tx.commit().unwrap();

    // List the blocks.
    let fetched_blocks = node_db::list_blocks(&conn, 0..100).unwrap();
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
    tx.commit().unwrap();

    // List the blocks by time.
    let start_time = Duration::from_secs(3);
    let end_time = Duration::from_secs(6);
    let fetched_blocks = node_db::list_blocks_by_time(&conn, start_time..end_time, 10, 0).unwrap();

    // Filter the original blocks to match the time range.
    let expected_blocks: Vec<_> = blocks
        .into_iter()
        .filter(|block| block.timestamp >= start_time && block.timestamp < end_time)
        .collect();

    assert_eq!(expected_blocks, fetched_blocks);
}

#[test]
fn test_list_contracts() {
    // The contract seeds for each block.
    let block_contract_seeds: &[&[Word]] = &[&[1], &[42, 69], &[1337, 7357, 9000], &[4]];

    // The list of contracts per block.
    let block_contracts: Vec<Vec<Contract>> = block_contract_seeds
        .iter()
        .map(|seeds| {
            seeds
                .iter()
                .copied()
                .map(util::test_contract)
                .collect::<Vec<_>>()
        })
        .collect();

    // Create an in-memory SQLite database.
    let mut conn = test_conn();

    // Create the necessary tables and insert contracts.
    let tx = conn.transaction().unwrap();
    node_db::create_tables(&tx).unwrap();
    for (ix, contracts) in block_contracts.iter().enumerate() {
        let block_n = ix.try_into().unwrap();
        for contract in contracts {
            node_db::insert_contract(&tx, contract, block_n).unwrap();
        }
    }
    tx.commit().unwrap();

    // Query the second and third blocks.
    let start = 1;
    let end = 3;

    // List the contracts.
    let fetched_contracts = node_db::list_contracts(&conn, start..end).unwrap();

    // Check the contracts per block match.
    let expected = &block_contracts[start as usize..end as usize];
    for ((ix, expected), (block, contracts)) in expected.iter().enumerate().zip(&fetched_contracts)
    {
        assert_eq!(ix as u64 + start, *block);
        assert_eq!(expected, contracts);
    }
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

    // Test queries at each block and solution.
    for block in &blocks {
        for (si, solution) in block.solutions.iter().enumerate() {
            for (di, data) in solution.data.iter().enumerate() {
                for (mi, mutation) in data.state_mutations.iter().enumerate() {
                    let state = node_db::finalized::query_state_inclusive_block(
                        &tx,
                        &contract_addr,
                        &mutation.key,
                        block.number,
                    )
                    .unwrap()
                    .unwrap();
                    assert_eq!(
                        state, block.solutions[2].data[di].state_mutations[mi].value,
                        "block: {}, sol: {}, data: {}, mut: {}, k: {:?}, v: {:?}",
                        block.number, si, di, mi, mutation.key, mutation.value
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
                            blocks[(block.number - 1) as usize].solutions[2].data[di]
                                .state_mutations[mi]
                                .value
                        );
                    }

                    let state = node_db::finalized::query_state_inclusive_solution(
                        &tx,
                        &contract_addr,
                        &mutation.key,
                        block.number,
                        si as u64,
                    )
                    .unwrap()
                    .unwrap();
                    assert_eq!(
                        state, mutation.value,
                        "block: {}, sol: {}, data: {}, mut: {}, k: {:?}, v: {:?}",
                        block.number, si, di, mi, mutation.key, mutation.value
                    );

                    let state = node_db::finalized::query_state_exclusive_solution(
                        &tx,
                        &contract_addr,
                        &mutation.key,
                        block.number,
                        si as u64,
                    )
                    .unwrap();

                    if block.number == 0 && si == 0 {
                        assert_eq!(state, None);
                    } else if si == 0 {
                        assert_eq!(
                            state.unwrap(),
                            blocks[(block.number - 1) as usize].solutions[2].data[di]
                                .state_mutations[mi]
                                .value
                        );
                    } else {
                        assert_eq!(
                            state.unwrap(),
                            block.solutions[si - 1].data[di].state_mutations[mi].value
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
        blocks.last().unwrap().solutions.last().unwrap().data[0].state_mutations[0].value
    );

    let state =
        node_db::finalized::query_state_inclusive_solution(&tx, &contract_addr, &vec![0], 9, 5)
            .unwrap()
            .unwrap();

    assert_eq!(
        state,
        blocks.last().unwrap().solutions.last().unwrap().data[0].state_mutations[0].value
    );

    let state =
        node_db::finalized::query_state_inclusive_solution(&tx, &contract_addr, &vec![0], 100, 100)
            .unwrap()
            .unwrap();

    assert_eq!(
        state,
        blocks.last().unwrap().solutions.last().unwrap().data[0].state_mutations[0].value
    );

    let state = node_db::finalized::query_state_exclusive_block(&tx, &contract_addr, &vec![0], 10)
        .unwrap()
        .unwrap();

    assert_eq!(
        state,
        blocks.last().unwrap().solutions.last().unwrap().data[0].state_mutations[0].value
    );

    let state =
        node_db::finalized::query_state_exclusive_solution(&tx, &contract_addr, &vec![0], 9, 5)
            .unwrap()
            .unwrap();

    assert_eq!(
        state,
        blocks.last().unwrap().solutions.last().unwrap().data[0].state_mutations[0].value
    );

    let state =
        node_db::finalized::query_state_exclusive_solution(&tx, &contract_addr, &vec![0], 100, 100)
            .unwrap()
            .unwrap();

    assert_eq!(
        state,
        blocks.last().unwrap().solutions.last().unwrap().data[0].state_mutations[0].value
    );
}
