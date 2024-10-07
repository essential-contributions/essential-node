#![allow(dead_code)]

use essential_types::{
    contract::Contract,
    predicate::Predicate,
    solution::{Mutation, Solution, SolutionData},
    Block, ConstraintBytecode, ContentAddress, PredicateAddress, StateReadBytecode, Word,
};
use std::time::Duration;

pub fn test_blocks_with_vars(n: u64) -> (ContentAddress, Vec<Block>) {
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
                    data.transient_data = values
                        .by_ref()
                        .take(2)
                        .zip(keys.by_ref())
                        .map(|(v, k)| Mutation {
                            key: vec![k as Word],
                            value: vec![v],
                        })
                        .collect();
                }
            }
            block
        })
        .collect::<Vec<_>>();
    (contract_addr, blocks)
}

pub fn test_blocks(n: u64) -> Vec<Block> {
    (0..n)
        .map(|i| test_block(i, Duration::from_secs(i as _)))
        .collect()
}

pub fn test_block(number: u64, timestamp: Duration) -> Block {
    let seed = number as i64 * 79;
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
        transient_data: vec![],
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
