#![allow(dead_code)]

use std::time::Duration;

use essential_types::{
    contract::Contract,
    predicate::Predicate,
    solution::{Solution, SolutionData},
    Block, ConstraintBytecode, PredicateAddress, StateReadBytecode,
};

pub fn test_blocks() -> Vec<Block> {
    (0..3)
        .map(|i| test_block(i, Duration::from_secs(i as _)))
        .collect()
}

pub fn test_block(number: u64, timestamp: Duration) -> Block {
    Block {
        number,
        timestamp,
        solutions: vec![test_solution(); 3],
    }
}

pub fn test_solution() -> Solution {
    Solution {
        data: vec![test_solution_data()],
    }
}

pub fn test_solution_data() -> SolutionData {
    let contract = test_contract();
    let predicate = essential_hash::content_addr(&contract.predicates[0]);
    let contract = essential_hash::contract_addr::from_contract(&contract);
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

pub fn test_contract() -> Contract {
    Contract {
        predicates: vec![test_predicate()],
        salt: [0; 32],
    }
}

pub fn test_predicate() -> Predicate {
    Predicate {
        state_read: test_state_reads(),
        constraints: test_constraints(),
        directive: essential_types::predicate::Directive::Satisfy,
    }
}

pub fn test_state_reads() -> Vec<StateReadBytecode> {
    vec![]
}

pub fn test_constraints() -> Vec<ConstraintBytecode> {
    vec![]
}
