//! The `Address` implementation for `Block` along with related fns for producing the block addr.

use super::{Block, Header};
use essential_hash::Address;
use essential_types::ContentAddress;

impl Address for Block {
    fn content_address(&self) -> ContentAddress {
        from_block(self)
    }
}

/// Shorthand for the common case of producing a block's content address
/// from a [`Block`].
///
/// *Note:* this also hashes each solution set.
/// If you already have the content address for each solution set, consider
/// using [`from_header_and_solution_set_addrs`] or [`from_header_and_solution_set_addrs_slice`].
pub fn from_block(block: &Block) -> ContentAddress {
    let solution_addrs = block.solution_sets.iter().map(essential_hash::content_addr);
    from_header_and_solution_set_addrs(&block.header, solution_addrs)
}

/// Given the content address for each solution set in the block, produce the
/// block's content address.
///
/// *Warning:* the caller **must** ensure that the order of the solution sets
/// matches the order of the solution sets in the block.
/// Otherwise the content address will be different then the one calculated
/// for the [`Block`].
pub fn from_header_and_solution_set_addrs(
    header: &Header,
    solution_set_addrs: impl IntoIterator<Item = ContentAddress>,
) -> ContentAddress {
    let solution_set_addrs: Vec<ContentAddress> = solution_set_addrs.into_iter().collect();
    from_header_and_solution_set_addrs_slice(header, &solution_set_addrs)
}

/// Given the content address for each solution set in the block, produce the
/// block's content address.
///
/// *Warning:* the caller **must** ensure that the order of the solution sets
/// matches the order of the solution sets in the block.
/// Otherwise the content address will be different then the one calculated
/// for the [`Block`].
pub fn from_header_and_solution_set_addrs_slice(
    header: &Header,
    solution_set_addrs: &[ContentAddress],
) -> ContentAddress {
    ContentAddress(essential_hash::hash(&(
        header.number,
        header.timestamp,
        solution_set_addrs,
    )))
}
