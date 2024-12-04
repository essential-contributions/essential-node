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
use essential_node_types::{register_contract_solution, register_program_solution, BigBang};
use essential_types::{
    contract::Contract,
    predicate::{Edge, Node, Predicate, PredicateEncodeError, Program, Reads},
    solution::{Mutation, Solution, SolutionSet},
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

pub fn test_blocks(n: Word) -> (Vec<Block>, Vec<Contract>, Vec<Program>) {
    let mut blocks = vec![];
    let mut contracts = vec![];
    let mut programs = vec![];
    for i in 0..n {
        let (block, inner_contracts, inner_programs) = test_block(i, Duration::from_secs(i as _));
        blocks.push(block);
        contracts.extend(inner_contracts);
        programs.extend(inner_programs);
    }

    (blocks, contracts, programs)
}

pub fn test_block_with_contracts(number: Word, timestamp: Duration) -> Block {
    let (mut block, contracts, programs) = test_block(number, timestamp);
    let big_bang = test_big_bang();
    let contract_registry = big_bang.contract_registry;
    let program_registry = big_bang.program_registry;
    let contracts_solution_set =
        register_contracts_solution_set(contract_registry, contracts.iter()).unwrap();
    let programs_solution_set = register_programs_solution_set(program_registry, programs.iter());
    match block.solution_sets.get_mut(0) {
        Some(first) => first.solutions.extend(
            contracts_solution_set
                .solutions
                .into_iter()
                .chain(programs_solution_set.solutions),
        ),
        None => block
            .solution_sets
            .extend(vec![contracts_solution_set, programs_solution_set]),
    }
    block
}

pub fn test_block(number: Word, timestamp: Duration) -> (Block, Vec<Contract>, Vec<Program>) {
    let seed = number * 3;
    let mut solution_sets = vec![];
    let mut contracts = vec![];
    let mut programs = vec![];
    for i in 0..3 {
        let (solution_set, contract, inner_programs) = test_solution_set(seed + i);
        solution_sets.push(solution_set);
        contracts.push(contract);
        programs.extend(inner_programs);
    }

    (
        Block {
            number,
            timestamp,
            solution_sets,
        },
        contracts,
        programs,
    )
}

pub fn test_invalid_block_with_contract(number: Word, timestamp: Duration) -> Block {
    let (mut block, contract, program) = test_invalid_block(number, timestamp);

    let big_bang = test_big_bang();
    let contract_registry = big_bang.contract_registry;
    let program_registry = big_bang.program_registry;
    let contract_solution_set =
        register_contracts_solution_set(contract_registry, Some(&contract)).unwrap();
    let programs_solution_set = register_programs_solution_set(program_registry, Some(&program));
    match block.solution_sets.get_mut(0) {
        Some(first) => first.solutions.extend(
            contract_solution_set
                .solutions
                .into_iter()
                .chain(programs_solution_set.solutions),
        ),
        None => block
            .solution_sets
            .extend(vec![contract_solution_set, programs_solution_set]),
    }
    block
}

pub fn test_invalid_block(number: Word, timestamp: Duration) -> (Block, Contract, Program) {
    let seed = number;

    let (predicate, program) = test_false_predicate(seed);
    let contract = Contract {
        predicates: vec![predicate],
        salt: essential_types::convert::u8_32_from_word_4([seed; 4]),
    };
    let predicate = essential_hash::content_addr(&contract.predicates[0]);
    let contract_address = essential_hash::content_addr(&contract);
    let solution = Solution {
        predicate_to_solve: PredicateAddress {
            contract: contract_address,
            predicate,
        },
        predicate_data: vec![],
        state_mutations: vec![Mutation {
            key: vec![seed],
            value: vec![0, 0, 0, 0],
        }],
    };
    let solution_set = SolutionSet {
        solutions: vec![solution],
    };

    (
        Block {
            number,
            timestamp,
            solution_sets: vec![solution_set],
        },
        contract,
        program,
    )
}

pub fn test_false_predicate(seed: Word) -> (Predicate, Program) {
    use essential_check::vm::asm::short::*;

    let a = Program(asm::to_bytes([PUSH(seed), POP, PUSH(0)]).collect());
    let a_ca = content_addr(&a);

    let nodes = vec![Node {
        program_address: a_ca,
        edge_start: Edge::MAX,
        reads: Reads::Pre,
    }];
    let edges = vec![];
    (Predicate { nodes, edges }, a)
}

pub fn test_solution_set(seed: Word) -> (SolutionSet, Contract, Vec<Program>) {
    let (solution, contract, programs) = test_solution(seed);
    (
        SolutionSet {
            solutions: vec![solution],
        },
        contract,
        programs,
    )
}

pub fn test_solution(seed: Word) -> (Solution, Contract, Vec<Program>) {
    let (contract, programs) = test_contract(seed);
    let predicate = essential_hash::content_addr(&contract.predicates[0]);
    let contract_address = essential_hash::content_addr(&contract);
    (
        Solution {
            predicate_to_solve: PredicateAddress {
                contract: contract_address,
                predicate,
            },
            predicate_data: vec![],
            state_mutations: vec![Mutation {
                key: vec![seed],
                value: vec![0, 0, 0, 0],
            }],
        },
        contract,
        programs,
    )
}

pub fn test_contract(seed: Word) -> (Contract, Vec<Program>) {
    // Make sure each predicate is unique, or contract will have a different
    // number of entries after insertion when multiple predicates have same CA.
    let n = 1 + seed % 2;
    let (predicates, programs) = (0..n)
        .map(|ix| test_predicate(seed * (2 + ix)))
        .collect::<(_, Vec<_>)>();
    let programs = programs.into_iter().flatten().collect();
    let contract = Contract {
        predicates,
        salt: essential_types::convert::u8_32_from_word_4([seed; 4]),
    };
    (contract, programs)
}

pub fn test_predicate(seed: Word) -> (Predicate, Vec<Program>) {
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
    (Predicate { nodes, edges }, vec![a, b, c])
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
        for solution_set in &block.solution_sets {
            for solution in &solution_set.solutions {
                for mutation in &solution.state_mutations {
                    let value = query_state_inclusive_block(
                        conn,
                        &solution.predicate_to_solve.contract,
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

/// A helper for constructing a solution set that registers the given set of contracts.
pub fn register_contracts_solution_set<'a>(
    contract_registry: PredicateAddress,
    contracts: impl IntoIterator<Item = &'a Contract>,
) -> Result<SolutionSet, PredicateEncodeError> {
    let solutions = contracts
        .into_iter()
        .map(|contract| register_contract_solution(contract_registry.clone(), contract))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(SolutionSet { solutions })
}

/// A helper for constructing a solution set that registers the given set of programs.
pub fn register_programs_solution_set<'a>(
    program_registry: PredicateAddress,
    programs: impl IntoIterator<Item = &'a Program>,
) -> SolutionSet {
    let solutions = programs
        .into_iter()
        .map(|program| register_program_solution(program_registry.clone(), program))
        .collect::<Vec<_>>();
    SolutionSet { solutions }
}

/// A helper for constructing a block that solely registers the given set of contracts.
pub fn register_contracts_block<'a>(
    contract_registry: PredicateAddress,
    contracts: impl IntoIterator<Item = &'a Contract>,
    block_number: Word,
    block_timestamp: Duration,
) -> Result<Block, PredicateEncodeError> {
    let solution_set = register_contracts_solution_set(contract_registry, contracts)?;
    Ok(Block {
        solution_sets: vec![solution_set],
        number: block_number,
        timestamp: block_timestamp,
    })
}
