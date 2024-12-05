use essential_node_types::{block_state_solution, register_contract_solution, BigBang};
use essential_types::{
    contract::Contract, predicate::Predicate, solution::SolutionSet, PredicateAddress,
};

// This function generates the default [`BigBang`].
//
// This makes it easier to keep the `big-bang.yml` up to date.
fn default_big_bang() -> BigBang {
    fn empty_predicate() -> Predicate {
        Predicate {
            nodes: vec![],
            edges: vec![],
        }
    }

    fn contract_registry_contract() -> Contract {
        Contract {
            salt: essential_hash::hash_bytes("contract_registry".as_bytes()),
            // TODO: Use a proper predicate that validates given predicates, etc.
            predicates: vec![empty_predicate()],
        }
    }

    fn program_registry_contract() -> Contract {
        Contract {
            salt: essential_hash::hash_bytes("program_registry".as_bytes()),
            // TODO:
            predicates: vec![empty_predicate()],
        }
    }

    fn block_state_contract() -> Contract {
        Contract {
            salt: essential_hash::hash_bytes("block_state".as_bytes()),
            // TODO:
            predicates: vec![empty_predicate()],
        }
    }

    let block_state = block_state_contract();
    let contract_registry = contract_registry_contract();
    let program_registry = program_registry_contract();
    let block_state_address = PredicateAddress {
        contract: essential_hash::content_addr(&block_state),
        predicate: essential_hash::content_addr(&block_state.predicates[0]),
    };
    let contract_registry_address = PredicateAddress {
        contract: essential_hash::content_addr(&contract_registry),
        predicate: essential_hash::content_addr(&contract_registry.predicates[0]),
    };
    let program_registry_address = PredicateAddress {
        contract: essential_hash::content_addr(&program_registry),
        predicate: essential_hash::content_addr(&program_registry.predicates[0]),
    };

    let solution_set = SolutionSet {
        solutions: vec![
            // A solution that adds the contract registry to itself.
            register_contract_solution(contract_registry_address.clone(), &contract_registry)
                .expect("big bang contract must be valid"),
            // A solution that registers the program registry contract.
            register_contract_solution(contract_registry_address.clone(), &program_registry)
                .expect("big bang contract must be valid"),
            // A solution that registers the block state contract.
            register_contract_solution(contract_registry_address.clone(), &block_state)
                .expect("big bang contract must be valid"),
            // A solution that sets the block state block number to 0, timestamp to 0.
            block_state_solution(block_state_address.clone(), 0, 0),
        ],
    };

    BigBang {
        block_state: block_state_address,
        contract_registry: contract_registry_address,
        program_registry: program_registry_address,
        solution_set,
    }
}

// A function that generates what should be in the `big-bang-block.yml`.
fn gen_big_bang_yml() -> String {
    let bbb = default_big_bang();
    let bbb_yml = serde_yaml::to_string(&bbb).expect("big bang block must be valid");
    println!("{bbb_yml}");
    let comment = r#"# Generated via the `gen_big_bang_yml()` fn in `crates/node-types/tests/big-bang.rs`.
# Run `cargo test` with `-- --nocapture` to see the expected format."#;
    let s = format!("{comment}\n{bbb_yml}");
    println!("{s}");
    s
}

#[test]
fn check_default_big_bang() {
    // Panics internally if the `big-bang.yml` is invalid.
    let _bbb = BigBang::default();
}

#[test]
fn big_bang_block_yml_matches_generated() {
    assert_eq!(essential_node_types::DEFAULT_BIG_BANG, gen_big_bang_yml());
}
