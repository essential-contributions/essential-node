//! Core types used within this implementation of the Essential protocol.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use essential_types::{
    contract::Contract, convert::{word_4_from_u8_32, word_from_bytes_slice}, predicate::header::PredicateError, solution::{Mutation, Solution, SolutionData}, Block, ContentAddress, PredicateAddress, Word
};
use serde::{Deserialize, Serialize};

/// The default big-bang configuration.
pub const DEFAULT_BIG_BANG: &str = include_str!("../../../big-bang.yml");

/// Describes how to construct the big-bang (aka "genesis") block.
#[derive(Clone, Debug, Eq, PartialEq, PartialOrd, Hash, Ord, Serialize, Deserialize)]
pub struct BigBang {
    /// The address of the contract used to track block state.
    ///
    /// This contract includes special keys for the block number and block timestamp. E.g.
    ///
    /// - `[0]` is the key for the block number, which is a `i64`.
    /// - `[1]` is the key for the block timestamp, which is a `i64` for seconds since
    ///   `UNIX_EPOCH`.
    pub block_state: ContentAddress,
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
    /// Predicate entries contain their length in bytes as an `int` and their fully byte-encoded
    /// form within a `int[]` with padding in the final word if necessary. E.g.
    ///
    /// - `[1, <predicate-ca>, 0]` to get the length bytes as `int`.
    /// - `[1, <predicate-ca>, 1]` gets the padded encoded data as `int[]`.
    pub contract_registry: ContentAddress,
    /// The `Solution` used to initialize arbitrary state for the big bang block.
    ///
    /// The primary purpose is setting the initial block state and registering the big bang
    /// contracts.
    ///
    /// If constructing a custom `BigBang` configuration, care must be taken to ensure that this
    /// `Solution` does actually register the aforementioned contracts correctly.
    pub solution: Solution,
}

impl BigBang {
    /// Produce the big bang [`Block`].
    pub fn block(&self) -> Block {
        Block {
            number: 0,
            timestamp: std::time::Duration::from_secs(0),
            solutions: vec![self.solution.clone()],
        }
    }
}

impl Default for BigBang {
    fn default() -> Self {
        serde_yaml::from_str(DEFAULT_BIG_BANG)
            .expect("default `big-bang-block.yml` must be valid (checked in tests)")
    }
}

/// Create a solution for registering the given contract at the given
pub fn register_contract_solution(
    registry_predicate: PredicateAddress,
    contract: &Contract,
) -> Result<SolutionData, PredicateError> {
    Ok(SolutionData {
        predicate_to_solve: registry_predicate,
        transient_data: vec![],
        decision_variables: vec![],
        state_mutations: register_contract_mutations(contract)?,
    })
}

/// Generate the mutations required to register a given contract within the big bang's "contract
/// registry" contract. This is useful for constructing contract deployment `Solution`s.
///
/// Learn more about the layout of state within the contract registry
pub fn register_contract_mutations(contract: &Contract) -> Result<Vec<Mutation>, PredicateError> {
    const CONTRACTS_PREFIX: Word = 0;
    const PREDICATES_PREFIX: Word = 1;

    let mut muts = vec![];

    // Add the mutations that register the contract's salt and length.
    let contract_ca = essential_hash::content_addr(contract);
    let contract_ca_w = word_4_from_u8_32(contract_ca.0);
    let salt_w = word_4_from_u8_32(contract.salt);
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
        let pred_ca_w = word_4_from_u8_32(pred_ca.0);

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
        let pred_bytes: Vec<u8> = pred.encode()?.collect();
        let len_bytes = pred_bytes.len();
        let len_bytes_w = Word::try_from(len_bytes).expect("checked during `encode`");

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

    Ok(muts)
}

fn padded_words_from_bytes(bytes: &[u8]) -> impl '_ + Iterator<Item = Word> {
    bytes
        .chunks(core::mem::size_of::<Word>())
        .map(word_from_bytes_slice)
}
