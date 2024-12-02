#![allow(dead_code)]

use crate::{
    db::{
        finalized::query_state_inclusive_block,
        get_validation_progress,
        pool::{Config, Source},
        ConnectionPool,
    },
    ensure_big_bang_block,
};
use essential_check::vm::asm;
use essential_hash::content_addr;
use essential_node_types::{register_contract_solution, BigBang};
use essential_types::{
    contract::Contract,
    predicate::{Edge, Node, Predicate, PredicateEncodeError, Program, Reads},
    solution::{Mutation, Solution, SolutionData},
    Block, ContentAddress, PredicateAddress, Word,
};
use rusqlite::Connection;
use std::time::Duration;

pub fn test_conn_pool() -> ConnectionPool {
    let conf = test_db_conf();
    ConnectionPool::with_tables(&conf).unwrap()
}

pub async fn test_conn_pool_with_big_bang() -> ConnectionPool {
    let conn_pool = test_conn_pool();
    ensure_big_bang_block(&conn_pool, &BigBang::default())
        .await
        .unwrap();
    conn_pool
}

pub fn test_db_conf() -> Config {
    Config {
        source: Source::Memory(uuid::Uuid::new_v4().into()),
        ..Default::default()
    }
}

pub fn test_big_bang() -> BigBang {
    BigBang::default()
}

/// The same as test_blocks, but includes solutions for deploying the contracts in their associated
/// blocks.
pub fn test_blocks_with_contracts(start: Word, end: Word) -> Vec<Block> {
    (start..end)
        .map(|i| test_block_with_contracts(i, Duration::from_secs(i as _)))
        .collect()
}

pub fn test_blocks(n: Word) -> (Vec<Block>, Vec<Contract>) {
    let (blocks, contracts) = (0..n)
        .map(|i| test_block(i, Duration::from_secs(i as _)))
        .unzip::<_, _, Vec<_>, Vec<_>>();
    (blocks, contracts.into_iter().flatten().collect())
}

pub fn test_block_with_contracts(number: Word, timestamp: Duration) -> Block {
    let (mut block, contracts) = test_block(number, timestamp);
    let contract_registry = test_big_bang().contract_registry;
    let solution = register_contracts_solution(contract_registry, contracts.iter()).unwrap();
    match block.solutions.get_mut(0) {
        Some(first) => first.data.extend(solution.data),
        None => block.solutions.push(solution),
    }
    block
}

pub fn test_block(number: Word, timestamp: Duration) -> (Block, Vec<Contract>) {
    let seed = number * 3;
    let (solutions, contracts) = (0..3)
        .map(|i| test_solution(seed + i))
        .unzip::<_, _, Vec<_>, Vec<_>>();

    (
        Block {
            number,
            timestamp,
            solutions,
        },
        contracts,
    )
}

pub fn test_invalid_block_with_contract(number: Word, timestamp: Duration) -> Block {
    let (mut block, contract) = test_invalid_block(number, timestamp);
    let contract_registry = test_big_bang().contract_registry;
    let solution = register_contracts_solution(contract_registry, Some(&contract)).unwrap();
    match block.solutions.get_mut(0) {
        Some(first) => first.data.extend(solution.data),
        None => block.solutions.push(solution),
    }
    block
}

pub fn test_invalid_block(number: Word, timestamp: Duration) -> (Block, Contract) {
    let seed = number;

    let predicate = test_invalid_predicate(seed);
    let contract = Contract {
        predicates: vec![predicate],
        salt: essential_types::convert::u8_32_from_word_4([seed; 4]),
    };
    let predicate = essential_hash::content_addr(&contract.predicates[0]);
    let contract_address = essential_hash::content_addr(&contract);
    let solution_data = SolutionData {
        predicate_to_solve: PredicateAddress {
            contract: contract_address,
            predicate,
        },
        decision_variables: vec![],
        state_mutations: vec![Mutation {
            key: vec![seed],
            value: vec![0, 0, 0, 0],
        }],
    };
    let solution = Solution {
        data: vec![solution_data],
    };

    (
        Block {
            number,
            timestamp,
            solutions: vec![solution],
        },
        contract,
    )
}

pub fn test_invalid_predicate(seed: Word) -> Predicate {
    use essential_asm::short::*;

    let a = Program(asm::to_bytes([PUSH(seed), POP, PUSH(0)]).collect());
    let a_ca = content_addr(&a);

    let nodes = vec![Node {
        program_address: a_ca,
        edge_start: Edge::MAX,
        reads: Reads::Pre,
    }];
    let edges = vec![];
    Predicate { nodes, edges }
}

pub fn test_solution(seed: Word) -> (Solution, Contract) {
    let (solution_data, contract) = test_solution_data(seed);
    (
        Solution {
            data: vec![solution_data],
        },
        contract,
    )
}

pub fn test_solution_data(seed: Word) -> (SolutionData, Contract) {
    let contract = test_contract(seed);
    let predicate = essential_hash::content_addr(&contract.predicates[0]);
    let contract_address = essential_hash::content_addr(&contract);
    (
        SolutionData {
            predicate_to_solve: PredicateAddress {
                contract: contract_address,
                predicate,
            },
            decision_variables: vec![],
            state_mutations: vec![Mutation {
                key: vec![seed],
                value: vec![0, 0, 0, 0],
            }],
        },
        contract,
    )
}

pub fn test_contract(seed: Word) -> Contract {
    let n = 1 + seed % 2;
    Contract {
        // Make sure each predicate is unique, or contract will have a different
        // number of entries after insertion when multiple predicates have same CA.
        predicates: (0..n).map(|ix| test_predicate(seed * (2 + ix))).collect(),
        salt: essential_types::convert::u8_32_from_word_4([seed; 4]),
    }
}

pub fn test_predicate(seed: Word) -> Predicate {
    use essential_asm::short::*;

    let a = Program(asm::to_bytes([PUSH(1), PUSH(2), PUSH(3), HLT]).collect());
    let b = Program(asm::to_bytes([PUSH(seed), HLT]).collect());
    let c = Program(
        asm::to_bytes([
            // Stack should already have `[1, 2, 3, seed]`.
            PUSH(1),
            PUSH(2),
            PUSH(3),
            PUSH(seed),
            // a `len` for `EqRange`.
            PUSH(4), // EqRange len
            EQRA,
            HLT,
        ])
        .collect(),
    );

    let a_ca = content_addr(&a);
    let b_ca = content_addr(&b);
    let c_ca = content_addr(&c);

    let node = |program_address, edge_start| Node {
        program_address,
        edge_start,
        reads: Reads::Pre, // unused for this test.
    };
    let nodes = vec![
        node(a_ca.clone(), 0),
        node(b_ca.clone(), 1),
        node(c_ca.clone(), Edge::MAX),
    ];
    let edges = vec![2, 2];
    Predicate { nodes, edges }
}

// Check that the validation progress in the database is block number and hash
pub fn assert_validation_progress_is_some(conn: &Connection, hash: &ContentAddress) {
    let progress_hash = get_validation_progress(conn)
        .unwrap()
        .expect("validation progress should be some");
    assert_eq!(progress_hash, *hash);
}

// Check that the validation in the database is none
pub fn assert_validation_progress_is_none(conn: &Connection) {
    assert!(get_validation_progress(conn).unwrap().is_none());
}

// Check state
pub fn assert_multiple_block_mutations(conn: &Connection, blocks: &[&Block]) {
    for block in blocks {
        for solution in &block.solutions {
            for data in &solution.data {
                for mutation in &data.state_mutations {
                    let value = query_state_inclusive_block(
                        conn,
                        &data.predicate_to_solve.contract,
                        &mutation.key,
                        block.number,
                    )
                    .unwrap()
                    .unwrap();
                    assert_eq!(value, mutation.value);
                }
            }
        }
    }
}

/// A helper for constructing a solution that registers the given set of contracts.
pub fn register_contracts_solution<'a>(
    contract_registry: PredicateAddress,
    contracts: impl IntoIterator<Item = &'a Contract>,
) -> Result<Solution, PredicateEncodeError> {
    let data = contracts
        .into_iter()
        .map(|contract| register_contract_solution(contract_registry.clone(), contract))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Solution { data })
}

/// A helper for constructing a block that solely registers the given set of contracts.
pub fn register_contracts_block<'a>(
    contract_registry: PredicateAddress,
    contracts: impl IntoIterator<Item = &'a Contract>,
    block_number: Word,
    block_timestamp: Duration,
) -> Result<Block, PredicateEncodeError> {
    let solution = register_contracts_solution(contract_registry, contracts)?;
    Ok(Block {
        solutions: vec![solution],
        number: block_number,
        timestamp: block_timestamp,
    })
}
