use essential_node_db::{self as node_db, BlockNumber, Optimistic, SolutionIndex, StatePosition};
use essential_types::{contract::Contract, solution::Mutation, ContentAddress, Word};
use rusqlite::Connection;
use std::time::Duration;

mod util;

#[test]
fn get_contract_salt() {
    // The test contract.
    let seed = 42;
    let da_block = 100;
    let contract = util::test_contract(seed);

    // Create an in-memory SQLite database.
    let mut conn = Connection::open_in_memory().unwrap();

    // Create the necessary tables and insert a contract.
    let tx = conn.transaction().unwrap();
    node_db::create_tables(&tx).unwrap();
    node_db::insert_contract(&tx, &contract, da_block).unwrap();
    tx.commit().unwrap();

    // Fetch the salt.
    let ca = essential_hash::contract_addr::from_contract(&contract);
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
    let mut conn = Connection::open_in_memory().unwrap();

    // Create the necessary tables and insert a contract.
    let tx = conn.transaction().unwrap();
    node_db::create_tables(&tx).unwrap();
    node_db::insert_contract(&tx, &contract, da_block).unwrap();
    tx.commit().unwrap();

    // Fetch the predicates.
    let ca = essential_hash::contract_addr::from_contract(&contract);
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
    let mut conn = Connection::open_in_memory().unwrap();

    // Create the necessary tables and insert a contract.
    let tx = conn.transaction().unwrap();
    node_db::create_tables(&tx).unwrap();
    node_db::insert_contract(&tx, &contract, da_block).unwrap();
    tx.commit().unwrap();

    // Fetch the contract.
    let ca = essential_hash::contract_addr::from_contract(&contract);
    let fetched_contract = node_db::get_contract(&conn, &ca).unwrap().unwrap();

    assert_eq!(contract, fetched_contract);
}

#[test]
fn test_get_contract_progress() {
    // Create an in-memory SQLite database.
    let mut conn = Connection::open_in_memory().unwrap();

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
    let mut conn = Connection::open_in_memory().unwrap();

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
    let mut conn = Connection::open_in_memory().unwrap();

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
    // Create an in-memory SQLite database.
    let mut conn = Connection::open_in_memory().unwrap();

    // Create the necessary tables and insert the contract progress.
    let tx = conn.transaction().unwrap();
    node_db::create_tables(&tx).unwrap();
    node_db::update_state_progress(&tx, 42, &ContentAddress([42; 32])).unwrap();
    tx.commit().unwrap();

    // Fetch the state progress.
    let (block_number, block_hash) = node_db::get_state_progress(&conn).unwrap().unwrap();

    assert_eq!(block_number, 42);
    assert_eq!(block_hash, ContentAddress([42; 32]));
}

#[test]
fn test_list_blocks() {
    // The test blocks.
    let blocks = util::test_blocks(100);

    // Create an in-memory SQLite database.
    let mut conn = Connection::open_in_memory().unwrap();

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
    let mut conn = Connection::open_in_memory().unwrap();

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
    let mut conn = Connection::open_in_memory().unwrap();

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
fn test_query_state_at() {
    let mut values = 0..Word::MAX;
    let contract_addr = ContentAddress([42; 32]);
    let blocks = util::test_blocks(10)
        .into_iter()
        .map(|mut block| {
            for solution in block.solutions.iter_mut() {
                for (k, data) in solution.data.iter_mut().enumerate() {
                    data.predicate_to_solve.contract = contract_addr.clone();
                    data.state_mutations = values
                        .by_ref()
                        .take(2)
                        .enumerate()
                        .map(|(d, v)| Mutation {
                            key: vec![((k + 1) * (d + 1)) as Word],
                            value: vec![v],
                        })
                        .collect();
                }
            }
            block
        })
        .collect::<Vec<_>>();

    // Create an in-memory SQLite database.
    let mut conn = Connection::open_in_memory().unwrap();
    let tx = conn.transaction().unwrap();
    node_db::create_tables(&tx).unwrap();
    for block in &blocks {
        node_db::insert_block(&tx, block).unwrap();
    }

    let pos = StatePosition::Optimistic(Optimistic {
        number: BlockNumber::Number(10),
        position: SolutionIndex::Index(0),
    });
    let state = node_db::query_state_at(&tx, &contract_addr, &vec![1], &pos)
        .unwrap()
        .unwrap();

    assert_eq!(
        state,
        blocks[9].solutions[2].data[0].state_mutations[0].value
    );

    let pos = StatePosition::Optimistic(Optimistic {
        number: BlockNumber::Number(9),
        position: SolutionIndex::Index(1),
    });
    let state = node_db::query_state_at(&tx, &contract_addr, &vec![1], &pos)
        .unwrap()
        .unwrap();
    assert_eq!(
        state,
        blocks[9].solutions[1].data[0].state_mutations[0].value
    );

    let pos = StatePosition::Optimistic(Optimistic {
        number: BlockNumber::Number(9),
        position: SolutionIndex::Index(0),
    });
    let state = node_db::query_state_at(&tx, &contract_addr, &vec![1], &pos)
        .unwrap()
        .unwrap();
    assert_eq!(
        state,
        blocks[9].solutions[0].data[0].state_mutations[0].value
    );

    let pos = StatePosition::Optimistic(Optimistic {
        number: BlockNumber::Number(8),
        position: SolutionIndex::Index(1),
    });
    let state = node_db::query_state_at(&tx, &contract_addr, &vec![1], &pos)
        .unwrap()
        .unwrap();
    assert_eq!(
        state,
        blocks[8].solutions[1].data[0].state_mutations[0].value
    );

    let pos = StatePosition::Optimistic(Optimistic {
        number: BlockNumber::Latest,
        position: SolutionIndex::Index(2),
    });
    let state = node_db::query_state_at(&tx, &contract_addr, &vec![1], &pos)
        .unwrap()
        .unwrap();
    assert_eq!(
        state,
        blocks[9].solutions[2].data[0].state_mutations[0].value
    );

    let pos = StatePosition::Optimistic(Optimistic {
        number: BlockNumber::Latest,
        position: SolutionIndex::Index(1),
    });
    let state = node_db::query_state_at(&tx, &contract_addr, &vec![1], &pos)
        .unwrap()
        .unwrap();
    assert_eq!(
        state,
        blocks[9].solutions[1].data[0].state_mutations[0].value
    );

    let pos = StatePosition::Optimistic(Optimistic {
        number: BlockNumber::Number(1),
        position: SolutionIndex::Start,
    });
    let state = node_db::query_state_at(&tx, &contract_addr, &vec![2], &pos)
        .unwrap()
        .unwrap();
    assert_eq!(
        state,
        blocks[0].solutions[2].data[0].state_mutations[1].value
    );

    let pos = StatePosition::Optimistic(Optimistic {
        number: BlockNumber::Number(0),
        position: SolutionIndex::Start,
    });
    let state = node_db::query_state_at(&tx, &contract_addr, &vec![2], &pos).unwrap();
    assert!(state.is_none());

    let pos = StatePosition::Optimistic(Optimistic {
        number: BlockNumber::Number(0),
        position: SolutionIndex::End,
    });
    let state = node_db::query_state_at(&tx, &contract_addr, &vec![2], &pos)
        .unwrap()
        .unwrap();
    assert_eq!(
        state,
        blocks[0].solutions[2].data[0].state_mutations[1].value
    );

    let pos = StatePosition::Optimistic(Optimistic {
        number: BlockNumber::Number(5),
        position: SolutionIndex::Index(20),
    });
    let state = node_db::query_state_at(&tx, &contract_addr, &vec![2], &pos)
        .unwrap()
        .unwrap();
    assert_eq!(
        state,
        blocks[5].solutions[2].data[0].state_mutations[1].value
    );

    // Add more blocks

    let new_blocks = util::test_blocks(20)
        .into_iter()
        .map(|mut block| {
            for solution in block.solutions.iter_mut() {
                for data in solution.data.iter_mut() {
                    data.predicate_to_solve.contract = contract_addr.clone();
                    data.state_mutations = vec![Mutation {
                        key: vec![99],
                        value: vec![42],
                    }];
                }
            }
            block
        })
        .collect::<Vec<_>>();
    for block in &new_blocks[10..] {
        node_db::insert_block(&tx, block).unwrap();
    }

    // Check the latest value is still the same
    let pos = StatePosition::Optimistic(Optimistic {
        number: BlockNumber::Latest,
        position: SolutionIndex::End,
    });
    let state = node_db::query_state_at(&tx, &contract_addr, &vec![1], &pos)
        .unwrap()
        .unwrap();
    assert_eq!(
        state,
        blocks[9].solutions[2].data[0].state_mutations[0].value
    );
}
