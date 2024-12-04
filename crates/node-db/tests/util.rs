#![allow(dead_code)]

use essential_check::vm::asm;
use essential_hash::content_addr;
use essential_node_db::{self as db, ConnectionPool};
use essential_node_types::{register_contract_solution, BigBang};
use essential_types::{
    contract::Contract,
    predicate::{Edge, Node, Predicate, PredicateEncodeError, Program, Reads},
    solution::{Mutation, Solution, SolutionData},
    Block, ContentAddress, PredicateAddress, Word,
};
use rusqlite::Connection;
use std::time::Duration;

pub fn test_conn() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", true).unwrap();
    conn
}

pub fn test_on_disk_conn(path: &str) -> Connection {
    let conn = Connection::open(path).unwrap();
    conn.pragma_update(None, "foreign_keys", true).unwrap();
    conn
}

pub fn test_pool_conf() -> db::pool::Config {
    db::pool::Config {
        source: db::pool::Source::Memory(uuid::Uuid::new_v4().into()),
        ..Default::default()
    }
}

pub fn test_conn_pool() -> ConnectionPool {
    let conf = test_pool_conf();
    ConnectionPool::with_tables(&conf).unwrap()
}

pub fn test_contract_registry() -> PredicateAddress {
    BigBang::default().contract_registry
}

pub fn test_blocks_with_vars(n: Word) -> (ContentAddress, Vec<Block>) {
    let mut values = 0..Word::MAX;
    let contract_addr = ContentAddress([42; 32]);
    let blocks = test_blocks(n)
        .into_iter()
        .map(|mut block| {
            for solution in block.solutions.iter_mut() {
                solution.data.push(test_solution_data(0));
                let mut keys = 0..Word::MAX;
                for data in &mut solution.data {
                    data.predicate_to_solve.contract = contract_addr.clone();
                    data.state_mutations = values
                        .by_ref()
                        .take(2)
                        .zip(keys.by_ref())
                        .map(|(v, k)| Mutation {
                            key: vec![k as Word],
                            value: vec![v],
                        })
                        .collect();
                    data.decision_variables = values.by_ref().take(2).map(|v| vec![v]).collect();
                }
            }
            block
        })
        .collect::<Vec<_>>();
    (contract_addr, blocks)
}

pub fn test_blocks(n: Word) -> Vec<Block> {
    (0..n)
        .map(|i| test_block(i, Duration::from_secs(i as _)))
        .collect()
}

pub fn test_block(number: Word, timestamp: Duration) -> Block {
    let seed = number * 79;
    Block {
        number,
        timestamp,
        solutions: (0..3).map(|i| test_solution(seed * (1 + i))).collect(),
    }
}

pub fn test_solution(seed: Word) -> Solution {
    Solution {
        data: vec![test_solution_data(seed)],
    }
}

pub fn test_solution_data(seed: Word) -> SolutionData {
    let contract = test_contract(seed);
    let predicate = essential_hash::content_addr(&contract.predicates[0]);
    let contract = essential_hash::content_addr(&contract);
    SolutionData {
        predicate_to_solve: PredicateAddress {
            contract,
            predicate,
        },
        decision_variables: vec![],
        state_mutations: vec![],
    }
}

pub fn test_pred_addr() -> PredicateAddress {
    PredicateAddress {
        contract: [0xAA; 32].into(),
        predicate: [0xAA; 32].into(),
    }
}

pub fn test_contract(seed: Word) -> Contract {
    let n = 1 + seed % 2;
    Contract {
        // Make sure each predicate is unique, or contract will have a different
        // number of entries after insertion when multiple predicates have same CA.
        predicates: (0..n).map(|ix| test_predicate(seed * (1 + ix))).collect(),
        salt: essential_types::convert::u8_32_from_word_4([seed; 4]),
    }
}

pub fn test_predicate(seed: Word) -> Predicate {
    use essential_check::vm::asm::short::*;

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
