//! The `Block` type and related implementations.

use core::time::Duration;
use essential_types::{SolutionSet, Word};
use serde::{Deserialize, Serialize};

pub mod addr;
#[cfg(test)]
mod tests;

/// An essential block.
#[derive(Clone, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Block {
    /// Metadata for the block.
    #[serde(flatten)]
    pub header: Header,
    /// The list of solution sets that make up a block.
    pub solution_sets: Vec<SolutionSet>,
}

/// The block header, containing metadata about the block.
#[derive(Clone, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Header {
    /// The block number.
    pub number: Word,
    /// The timestamp at which the
    pub timestamp: Duration,
}
