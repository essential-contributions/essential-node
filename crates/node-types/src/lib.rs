//! Core types used within this implementation of the Essential protocol.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use essential_types::{
    contract::Contract,
    convert::{word_4_from_u8_32, word_from_bytes_slice},
    predicate::{PredicateEncodeError, Program},
    solution::{Mutation, Solution, SolutionSet},
    Block, PredicateAddress, Word,
};
use serde::{Deserialize, Serialize};

/// Wrappers around tokio's `watch` channel for notifying of new blocks.
#[cfg(feature = "block-notify")]
pub mod block_notify;

/// The default big-bang configuration.
pub const DEFAULT_BIG_BANG: &str = include_str!("../big-bang.yml");

/// Describes how to construct the big-bang (aka "genesis") block.
#[derive(Clone, Debug, Eq, PartialEq, PartialOrd, Hash, Ord, Serialize, Deserialize)]
pub struct BigBang {
    /// The address of the contract used to track block state.
    ///
    /// This contract includes special keys for the block number and block timestamp. E.g.
    ///
    /// - `[0]` is the key for the block number, which is a `Word`.
    /// - `[1]` is the key for the block timestamp, which is a `Word` for seconds since
    ///   `UNIX_EPOCH`.
    pub block_state: PredicateAddress,
    /// The address of the contract used to register contracts and their associated predicates.
    ///
    /// There are two primary regions of storage for the contract registry. The layout can be
    /// thought of as the following.
    ///
    /// ```ignore
    /// storage {
    ///     contracts: (b256 => Contract),
    ///     predicates: (b256 => Predicate),
    /// }
    /// ```
    ///
    /// - `contracts` have key prefix `[0]`
    /// - `predicates` have key prefix `[1]`
    ///
    /// ## Contracts
    ///
    /// Contract entries contain the salt and the addresses of its predicates. E.g.
    ///
    /// - `[0, <contract-ca>, 0]` is the key to the "salt", a `b256`.
    /// - `[0, <contract-ca>, <predicate-ca>]` is a key whose non-empty value specifies that the
    ///   predicate with the given address is associated with the contract.
    ///
    /// ## Predicates
    ///
    /// Predicate entries contain their length in bytes as a `Word` and their fully byte-encoded
    /// form within a `int[]` with padding in the final word if necessary. E.g.
    ///
    /// - `[1, <predicate-ca>]` to get the length bytes as `Word` followed by the fully encoded
    ///   word-padded data as `int[]`.
    pub contract_registry: PredicateAddress,
    /// The address of the contract used to register programs.
    ///
    /// ```ignore
    /// storage {
    ///     programs: (b256 => Program),
    /// }
    /// ```
    ///
    /// ## Programs
    ///
    /// Program entries contain their length in bytes as a `Word` and their bytecode within a
    /// `int[]` with padding in the final word if necessary. E.g.
    ///
    /// - `[0, <program-ca>]` to get the bytecode length as `Word` followed by the bytecode as
    ///   `int[]`.
    pub program_registry: PredicateAddress,
    /// The `SolutionSet` used to initialize arbitrary state for the big bang block.
    ///
    /// The primary purpose is setting the initial block state and registering the big bang
    /// contracts.
    ///
    /// If constructing a custom `BigBang` configuration, care must be taken to ensure that this
    /// `SolutionSet` does actually register the aforementioned contracts correctly.
    pub solution_set: SolutionSet,
}

impl BigBang {
    /// Produce the big bang [`Block`].
    pub fn block(&self) -> Block {
        Block {
            number: 0,
            timestamp: std::time::Duration::from_secs(0),
            solution_sets: vec![self.solution_set.clone()],
        }
    }
}

impl Default for BigBang {
    fn default() -> Self {
        serde_yaml::from_str(DEFAULT_BIG_BANG)
            .expect("default `big-bang-block.yml` must be valid (checked in tests)")
    }
}

/// Functions for constructing keys into the "contract registry" contract state.
pub mod contract_registry {
    use crate::padded_words_from_bytes;
    use essential_types::{ContentAddress, Key, PredicateAddress, Word};

    const CONTRACTS_PREFIX: Word = 0;
    const PREDICATES_PREFIX: Word = 1;

    /// A key that may be used to refer to a contract's `salt` in state.
    ///
    /// The returned key is formatted as: `[0, <contract-ca>, 0]`
    pub fn contract_salt_key(contract_ca: &ContentAddress) -> Key {
        Some(CONTRACTS_PREFIX)
            .into_iter()
            .chain(padded_words_from_bytes(&contract_ca.0))
            .chain(Some(0))
            .collect()
    }

    /// A key that may be used to test if the predicate exists within the contract specified in the
    /// `PredicateAddress`.
    ///
    /// The returned key is formatted as: `[0, <contract-ca>, <predicate-ca>]`
    pub fn contract_predicate_key(pred_addr: &PredicateAddress) -> Key {
        Some(CONTRACTS_PREFIX)
            .into_iter()
            .chain(padded_words_from_bytes(&pred_addr.contract.0))
            .chain(padded_words_from_bytes(&pred_addr.predicate.0))
            .collect()
    }

    /// A key that may be used to retrieve the full `Predicate` from the contract registry state.
    ///
    /// When queried, the `Predicate` data will be preceded by a single word that describes the
    /// length of the predicate in bytes.
    ///
    /// The returned key is formatted as: `[1, <predicate-ca>]`
    pub fn predicate_key(pred_ca: &ContentAddress) -> Key {
        Some(PREDICATES_PREFIX)
            .into_iter()
            .chain(padded_words_from_bytes(&pred_ca.0))
            .collect()
    }
}

/// Functions for constructing keys into the "program registry" contract state.
pub mod program_registry {
    use crate::padded_words_from_bytes;
    use essential_types::{ContentAddress, Key, Word};

    const PROGRAMS_PREFIX: Word = 0;

    /// A key that may be used to retrieve the full `Program` from the program registry state.
    ///
    /// When queried, the `Program` data will be preceded by a single word that describes the
    /// length of the program in bytes.
    ///
    /// The returned key is formatted as `[0, <program-ca>]`
    pub fn program_key(prog_ca: &ContentAddress) -> Key {
        Some(PROGRAMS_PREFIX)
            .into_iter()
            .chain(padded_words_from_bytes(&prog_ca.0))
            .collect()
    }
}

/// Create a solution for registering the given contract at the given contract registry.
pub fn register_contract_solution(
    contract_registry: PredicateAddress,
    contract: &Contract,
) -> Result<Solution, PredicateEncodeError> {
    Ok(Solution {
        predicate_to_solve: contract_registry,
        predicate_data: vec![],
        state_mutations: register_contract_mutations(contract)?,
    })
}

/// Generate the mutations required to register a given contract within the big bang's "contract
/// registry" contract. This is useful for constructing contract deployment `Solution`s.
///
/// Learn more about the layout of state within the contract registry in
/// [`BigBang::contract_registry`].
pub fn register_contract_mutations(
    contract: &Contract,
) -> Result<Vec<Mutation>, PredicateEncodeError> {
    let mut muts = vec![];

    // Add the mutations that register the contract's salt and length.
    let contract_ca = essential_hash::content_addr(contract);
    let salt_w = word_4_from_u8_32(contract.salt);

    // Add the salt at `[0, <contract-ca>, 0]`.
    muts.push(Mutation {
        key: contract_registry::contract_salt_key(&contract_ca),
        value: salt_w.to_vec(),
    });

    // Register the predicates.
    for pred in &contract.predicates {
        let pred_ca = essential_hash::content_addr(pred);
        let pred_addr = PredicateAddress {
            contract: contract_ca.clone(),
            predicate: pred_ca,
        };

        // Add to the contract `[0, <contract-addr>, <pred-addr>]`
        let key = contract_registry::contract_predicate_key(&pred_addr);
        muts.push(Mutation {
            key,
            value: vec![1],
        });

        // Encode the predicate so that it may be registered.
        let pred_bytes: Vec<u8> = pred.encode()?.collect();
        let len_bytes = pred_bytes.len();
        let len_bytes_w = Word::try_from(len_bytes).expect("checked during `encode`");

        // Add the encoded predicate.
        let key = contract_registry::predicate_key(&pred_addr.predicate);
        muts.push(Mutation {
            key,
            value: Some(len_bytes_w)
                .into_iter()
                .chain(padded_words_from_bytes(&pred_bytes))
                .collect(),
        });
    }

    Ok(muts)
}

/// Create a solution for registering the given program at the given program registry.
pub fn register_program_solution(
    program_registry: PredicateAddress,
    program: &Program,
) -> Solution {
    Solution {
        predicate_to_solve: program_registry,
        predicate_data: vec![],
        state_mutations: register_program_mutations(program),
    }
}

/// Generate the mutations required to register a given program within the big bang's "program
/// registry" contract. This is useful for constructing program deployment `Solution`s.
///
/// Learn more about the layout of state within the contract registry in
/// [`BigBang::program_registry`].
pub fn register_program_mutations(program: &Program) -> Vec<Mutation> {
    let mut muts = vec![];

    // Register the program.
    let prog_ca = essential_hash::content_addr(program);
    let prog_bytes = &program.0;
    let len_bytes = prog_bytes.len();
    // FIXME: relevant issue https://github.com/essential-contributions/essential-base/issues/240
    let len_bytes_w = Word::try_from(len_bytes).expect("should be enforced by Program::MAX_SIZE");

    // Add to the program `[0, <program-ca>]`
    let key = program_registry::program_key(&prog_ca);
    muts.push(Mutation {
        key,
        value: Some(len_bytes_w)
            .into_iter()
            .chain(padded_words_from_bytes(prog_bytes))
            .collect(),
    });

    muts
}

/// Generate a solution that sets the block state to the given block number and timestamp.
pub fn block_state_solution(
    block_state: PredicateAddress,
    block_number: Word,
    block_timestamp_secs: Word,
) -> Solution {
    Solution {
        predicate_to_solve: block_state,
        predicate_data: vec![],
        state_mutations: block_state_mutations(block_number, block_timestamp_secs),
    }
}

/// Generate the mutations required for a solution that sets the block state to the given block
/// number and timestamp.
pub fn block_state_mutations(block_number: Word, block_timestamp_secs: Word) -> Vec<Mutation> {
    vec![
        Mutation {
            key: vec![0],
            value: vec![block_number],
        },
        Mutation {
            key: vec![1],
            value: vec![block_timestamp_secs],
        },
    ]
}

fn padded_words_from_bytes(bytes: &[u8]) -> impl '_ + Iterator<Item = Word> {
    bytes
        .chunks(core::mem::size_of::<Word>())
        .map(word_from_bytes_slice)
}
