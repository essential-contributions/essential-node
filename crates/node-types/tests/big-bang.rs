use essential_node_types::{register_contract_mutations, BigBang};
use essential_types::{
    contract::Contract,
    predicate::Predicate,
    solution::{Mutation, Solution, SolutionData},
    PredicateAddress,
};

// This function generates the default [`BigBang`].
//
// This makes it easier to keep the `big-bang.yml` up to date.
fn default_big_bang() -> BigBang {
    fn empty_predicate() -> Predicate {
        Predicate {
            state_read: vec![],
            constraints: vec![],
        }
    }

    fn contract_registry_contract() -> Contract {
        Contract {
            salt: essential_hash::hash_bytes("contract_registry".as_bytes()),
            // TODO: Use a proper predicate that validates given predicates, etc.
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
    let block_state_address = essential_hash::content_addr(&block_state);
    let contract_registry_address = essential_hash::content_addr(&contract_registry);
    let register_contract_predicate_address = PredicateAddress {
        contract: contract_registry_address.clone(),
        predicate: essential_hash::content_addr(&contract_registry.predicates[0]),
    };
    let block_state_predicate_address = PredicateAddress {
        contract: block_state_address.clone(),
        predicate: essential_hash::content_addr(&block_state.predicates[0]),
    };
    let solution = Solution {
        data: vec![
            // A solution that adds the contract registry to itself.
            SolutionData {
                predicate_to_solve: register_contract_predicate_address.clone(),
                decision_variables: vec![],
                transient_data: vec![],
                state_mutations: register_contract_mutations(&contract_registry),
            },
            // A solution that registers the block state contract.
            SolutionData {
                predicate_to_solve: register_contract_predicate_address,
                decision_variables: vec![],
                transient_data: vec![],
                state_mutations: register_contract_mutations(&block_state),
            },
            // A solution that sets the block state block number to 0, timestamp to 0.
            SolutionData {
                predicate_to_solve: block_state_predicate_address,
                decision_variables: vec![],
                transient_data: vec![],
                state_mutations: vec![
                    Mutation {
                        key: vec![0],
                        value: vec![0],
                    },
                    Mutation {
                        key: vec![1],
                        value: vec![0],
                    },
                ],
            },
        ],
    };

    BigBang {
        block_state_address,
        contract_registry_address,
        solution,
    }
}

// A function that generates what should be in the `big-bang-block.yml`.
fn gen_big_bang_yml() -> String {
    let bbb = default_big_bang();
    let bbb_yml = serde_yaml::to_string(&bbb).expect("big bang block must be valid");
    println!("{bbb_yml}");
    let comment = r#"# Generated via the `gen_big_bang_block_yml()` fn in `crates/node-types/tests/big-bang.rs`.
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
