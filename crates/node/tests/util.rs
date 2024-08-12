#![allow(dead_code)]

use essential_node::stream::GetConn;
use essential_node_db::insert_contract;
use essential_types::{
    contract::Contract,
    predicate::Predicate,
    solution::{Mutation, Solution, SolutionData},
    Block, ConstraintBytecode, PredicateAddress, StateReadBytecode, Word,
};
use rusqlite::{Connection, OpenFlags};
use std::{future::Future, time::Duration};

#[derive(Clone, Copy)]
pub struct Conn;

impl GetConn for Conn {
    type Error = rusqlite::Error;
    type Connection = rusqlite::Connection;

    fn get(
        &self,
    ) -> impl Future<Output = std::result::Result<Self::Connection, Self::Error>> + Send {
        let mut flags = OpenFlags::default();
        flags.insert(OpenFlags::SQLITE_OPEN_SHARED_CACHE);
        let r = rusqlite::Connection::open_with_flags("file::memory:", flags).map_err(Into::into);
        futures::future::ready(r)
    }
}

pub fn test_blocks(conn: &mut Option<&mut Connection>, n: u64) -> Vec<Block> {
    (0..n)
        .map(|i| test_block(conn, i, Duration::from_secs(i as _)))
        .collect()
}

pub fn test_block(conn: &mut Option<&mut Connection>, number: u64, timestamp: Duration) -> Block {
    let seed = number as i64 * 79;
    Block {
        number,
        timestamp,
        solutions: (0..3)
            .map(|i| test_solution(conn, seed * (1 + i)))
            .collect(),
    }
}

pub fn test_solution(conn: &mut Option<&mut Connection>, seed: Word) -> Solution {
    Solution {
        data: vec![test_solution_data(conn, seed)],
    }
}

pub fn test_solution_data(conn: &mut Option<&mut Connection>, seed: Word) -> SolutionData {
    let contract = test_contract(seed);

    if let Some(conn) = conn {
        let tx = conn.transaction().unwrap();
        insert_contract(&tx, &contract, 0).unwrap();
        tx.commit().unwrap();
    }

    let predicate = essential_hash::content_addr(&contract.predicates[0]);
    let contract = essential_hash::contract_addr::from_contract(&contract);
    SolutionData {
        predicate_to_solve: PredicateAddress {
            contract,
            predicate,
        },
        decision_variables: vec![],
        transient_data: vec![],
        state_mutations: vec![Mutation {
            key: vec![0, 0, 0, 0],
            value: vec![seed],
        }],
    }
}

pub fn test_pred_addr() -> PredicateAddress {
    PredicateAddress {
        contract: [0xAA; 32].into(),
        predicate: [0xAA; 32].into(),
    }
}

pub fn test_contract(seed: Word) -> Contract {
    let n = (1 + seed % 2) as usize;
    Contract {
        // Make sure each predicate is unique, or contract will have a different
        // number of entries after insertion when multiple predicates have same CA.
        predicates: (0..n)
            .map(|ix| test_predicate(seed * (1 + ix as i64)))
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
    let n = (1 + seed % 3) as usize;
    let b = (seed % u8::MAX as Word) as u8;
    vec![vec![b; 10]; n]
}

// Resulting bytecode is invalid, but this is just for testing DB behaviour, not validation.
pub fn test_constraints(seed: Word) -> Vec<ConstraintBytecode> {
    let n = (1 + seed % 3) as usize;
    let b = (seed % u8::MAX as Word) as u8;
    vec![vec![b; 10]; n]
}
