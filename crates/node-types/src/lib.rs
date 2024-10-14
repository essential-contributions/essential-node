//! Core types used within this implementation of the Essential protocol.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use essential_types::{solution::Solution, ContentAddress};
use serde::{Deserialize, Serialize};

/// Describes the big-bang (aka "genesis") state of the essential node blockchain.
#[derive(Clone, Debug, Eq, PartialEq, PartialOrd, Hash, Ord, Serialize, Deserialize)]
pub struct BigBangBlock {
    /// The address of the contract used to track block state.
    ///
    /// This contract includes special keys for the block number and block timestamp.
    pub block_state_address: ContentAddress,
    /// The address of the contract used to register contracts and their associated predicates.
    pub contract_registry_address: ContentAddress,
    /// Specifies the initial state for both the block state and contract registry contracts.
    pub solution: Solution,
}
