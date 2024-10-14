//! Core types used within this implementation of the Essential protocol.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use essential_types::{solution::Solution, ContentAddress};
use serde::{Deserialize, Serialize};

/// The default big-bang-block configuration.
pub const DEFAULT_BIG_BANG_BLOCK: &str = include_str!("../../../big-bang-block.yml");

/// Describes how to construct the big-bang (aka "genesis") block.
#[derive(Clone, Debug, Eq, PartialEq, PartialOrd, Hash, Ord, Serialize, Deserialize)]
pub struct BigBangBlock {
    /// The address of the contract used to track block state.
    ///
    /// This contract includes special keys for the block number and block timestamp. E.g.
    ///
    /// - `[0]` is the key for the block number, which is a `i64`.
    /// - `[1]` is the key for the block timestamp, which is a `i64` for seconds since
    ///   `UNIX_EPOCH`.
    pub block_state_address: ContentAddress,
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
    /// predicate with the given address is associated with the contract.
    ///
    /// ## Predicates
    ///
    /// Predicate entries contain their length in bytes as an `int` and their fully byte-encoded
    /// form within a `int[]` with padding in the final word if necessary. E.g.
    ///
    /// - `[1, <predicate-ca>, 0]` to get the length bytes as `int`.
    /// - `[1, <predicate-ca>, 1]` gets the padded encoded data as `int[]`.
    pub contract_registry_address: ContentAddress,
    /// Specifies the initial state for both the block state and contract registry contracts.
    pub solution: Solution,
}

impl Default for BigBangBlock {
    fn default() -> Self {
        serde_yaml::from_str(DEFAULT_BIG_BANG_BLOCK)
            .expect("default `big-bang-block.yml` must be valid (checked in tests)")
    }
}
