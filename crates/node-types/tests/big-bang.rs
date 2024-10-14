use essential_node_types::BigBangBlock;
use essential_types::{
    contract::Contract,
    convert::{word_4_from_u8_32, word_from_bytes_slice},
    predicate::Predicate,
    solution::{Mutation, Solution, SolutionData},
    PredicateAddress, Word,
};

// This function generates the default [`BigBangBlock`].
//
// This makes it easier to keep the `big-bang-block.yml` up to date.
fn default_big_bang_block() -> BigBangBlock {
    fn padded_words_from_bytes(bytes: &[u8]) -> impl '_ + Iterator<Item = Word> {
        bytes
            .chunks(core::mem::size_of::<Word>())
            .map(move |chunk| word_from_bytes_slice(chunk))
    }

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

    // Generate mutations required to register a contract.
    fn register_contract_mutations(contract: &Contract) -> Vec<Mutation> {
        const CONTRACTS_PREFIX: Word = 0;
        const PREDICATES_PREFIX: Word = 1;

        let mut muts = vec![];

        // Add the mutations that register the contract's salt and length.
        let contract_ca = essential_hash::content_addr(contract);
        let contract_ca_w = word_4_from_u8_32(contract_ca.0.clone());
        let salt_w = word_4_from_u8_32(contract.salt.clone());
        let contract_key: Vec<_> = Some(CONTRACTS_PREFIX)
            .into_iter()
            .chain(contract_ca_w)
            .collect();

        // Add the salt at `[0, <contract-ca>, 0]`.
        muts.push(Mutation {
            key: contract_key.iter().copied().chain(Some(0)).collect(),
            value: salt_w.to_vec(),
        });

        // Register the predicates.
        for pred in &contract.predicates {
            let pred_ca = essential_hash::content_addr(pred);
            let pred_ca_w = word_4_from_u8_32(pred_ca.0.clone());

            // Add to the contract `[0, <contract-addr>, <pred-addr>]`
            muts.push(Mutation {
                key: contract_key.iter().copied().chain(pred_ca_w).collect(),
                value: vec![1],
            });

            // Encode the predicate so that it may be registered.
            let pred_key: Vec<_> = Some(PREDICATES_PREFIX)
                .into_iter()
                .chain(pred_ca_w)
                .collect();
            let pred_bytes: Vec<u8> = pred
                .encode()
                .expect("statically known predicate must be valid")
                .collect();
            let len_bytes = pred_bytes.len();
            let len_bytes_w =
                Word::try_from(len_bytes).expect("static contract must be in size range");

            // Add the `len` mutation.
            muts.push(Mutation {
                key: pred_key.iter().copied().chain(Some(0)).collect(),
                value: vec![len_bytes_w],
            });

            // Add the encoded predicate.
            muts.push(Mutation {
                key: pred_key.iter().copied().chain(Some(1)).collect(),
                value: padded_words_from_bytes(&pred_bytes).collect(),
            });
        }

        muts
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

    BigBangBlock {
        block_state_address,
        contract_registry_address,
        solution,
    }
}

// A function that generates what should be in the `big-bang-block.yml`.
fn gen_big_bang_block_yml() -> String {
    let bbb = default_big_bang_block();
    let bbb_yml = serde_yaml::to_string(&bbb).expect("big bang block must be valid");
    println!("{bbb_yml}");
    let comment = r#"# Generated via the `gen_big_bang_block_yml()` fn in `crates/node-types/tests/big-bang.rs`.
# Run `cargo test` with `-- --nocapture` to see the expected format."#;
    let s = format!("{comment}\n{bbb_yml}");
    println!("{s}");
    s
}

#[test]
fn check_default_big_bang_block() {
    // Panics internally if the `big-bang-block.yml` is invalid.
    let _bbb = BigBangBlock::default();
}

#[test]
fn big_bang_block_yml_matches_generated() {
    assert_eq!(
        essential_node_types::DEFAULT_BIG_BANG_BLOCK,
        gen_big_bang_block_yml(),
    );
}
