#![allow(dead_code)]

use crate::db::ConnectionPool;
use essential_node_db::{get_state_progress, get_validation_progress, query_state};
use essential_types::{
    contract::Contract,
    predicate::Predicate,
    solution::{Mutation, Solution, SolutionData},
    Block, ConstraintBytecode, ContentAddress, PredicateAddress, StateReadBytecode, Word,
};
use rusqlite::Connection;
use std::time::Duration;

pub fn test_conn_pool() -> ConnectionPool {
    let conf = test_db_conf();
    crate::db(&conf).unwrap()
}

pub fn test_db_conf() -> crate::db::Config {
    crate::db::Config::default().with_source(crate::db::Source::Memory(uuid::Uuid::new_v4().into()))
}

pub fn test_blocks(n: u64) -> (Vec<Block>, Vec<Contract>) {
    let (blocks, contracts) = (0..n)
        .map(|i| test_block(i, Duration::from_secs(i as _)))
        .unzip::<_, _, Vec<_>, Vec<_>>();
    (blocks, contracts.into_iter().flatten().collect())
}

pub fn test_block(number: u64, timestamp: Duration) -> (Block, Vec<Contract>) {
    let seed = number as i64 * 3;
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

pub fn test_invalid_block(number: u64, timestamp: Duration) -> (Block, Contract) {
    let seed = number as i64;

    let predicate = Predicate {
        state_read: test_state_reads(seed),
        constraints: vec![essential_constraint_asm::to_bytes(vec![
            essential_constraint_asm::Stack::Push(seed).into(),
            essential_constraint_asm::Stack::Pop.into(),
            // This constraint will fail
            essential_constraint_asm::Stack::Push(0).into(),
        ])
        .collect()],
        directive: essential_types::predicate::Directive::Satisfy,
    };
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
        transient_data: vec![],
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
            transient_data: vec![],
            state_mutations: vec![Mutation {
                key: vec![seed],
                value: vec![0, 0, 0, 0],
            }],
        },
        contract,
    )
}

pub fn test_contract(seed: Word) -> Contract {
    let n = (1 + seed % 2) as usize;
    Contract {
        // Make sure each predicate is unique, or contract will have a different
        // number of entries after insertion when multiple predicates have same CA.
        predicates: (0..n)
            .map(|ix| test_predicate(seed * (2 + ix as i64)))
            .collect(),
        salt: essential_types::convert::u8_32_from_word_4([seed; 4]),
    }
}

pub fn test_predicate(seed: Word) -> Predicate {
    Predicate {
        state_read: test_state_reads(seed),
        constraints: test_constraints(seed),
        directive: essential_types::predicate::Directive::Satisfy,
    }
}

// Resulting bytecode is invalid, but this is just for testing DB behaviour, not validation.
pub fn test_state_reads(seed: Word) -> Vec<StateReadBytecode> {
    vec![essential_state_asm::to_bytes(vec![
        essential_state_asm::Stack::Push(seed).into(),
        essential_state_asm::Stack::Pop.into(),
        essential_state_asm::TotalControlFlow::Halt.into(),
    ])
    .collect()]
}

// Resulting bytecode is invalid, but this is just for testing DB behaviour, not validation.
pub fn test_constraints(seed: Word) -> Vec<ConstraintBytecode> {
    vec![essential_constraint_asm::to_bytes(vec![
        essential_constraint_asm::Stack::Push(seed).into(),
        essential_constraint_asm::Stack::Pop.into(),
        essential_constraint_asm::Stack::Push(1).into(),
    ])
    .collect()]
}

// Check that the state progress in the database is block number and hash
pub fn assert_state_progress_is_some(conn: &Connection, hash: &ContentAddress) {
    let progress_hash = get_state_progress(conn)
        .unwrap()
        .expect("state progress should be some");
    assert_eq!(progress_hash, *hash);
}

// Check that the state progress in the database is none
pub fn assert_state_progress_is_none(conn: &Connection) {
    assert!(get_state_progress(conn).unwrap().is_none());
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
                    let value = query_state(conn, &data.predicate_to_solve.contract, &mutation.key)
                        .unwrap()
                        .unwrap();
                    assert_eq!(value, mutation.value);
                }
            }
        }
    }
}
