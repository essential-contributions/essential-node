//! Addressed queries query for the most recent version of a key less than or equal to a
//! given block address or solution set index. This is useful for querying un-finalized blocks
//! where forks may exist. These queries fall back to finalized queries if the value is not found.

use essential_node_db_sql as sql;
use essential_types::{ContentAddress, Key, Value, Word};
use rusqlite::{named_params, Connection, OptionalExtension, Transaction};

use crate::{blob_from_words, words_from_blob, QueryError};

/// Query the most recent value for a key in a contract's state
/// that was set at or before the given block address.
///
/// This is inclusive of the block's state (..=block_address).
pub fn query_state_inclusive_block(
    conn: &Connection,
    contract_ca: &ContentAddress,
    key: &Key,
    block_address: &ContentAddress,
) -> Result<Option<Value>, QueryError> {
    let mut stmt = conn.prepare(sql::query::QUERY_STATE_BLOCK_ADDRESS)?;
    let value_blob: Option<(Option<Vec<u8>>, Word)> = stmt
        .query_row(
            named_params! {
                ":contract_ca": contract_ca.0,
                ":key": blob_from_words(key),
                ":block_address": block_address.0,
                ":solution_set_index": None::<u64>,
            },
            |row| {
                let value = row.get("found_value")?;
                let number = row.get("number")?;
                Ok((value, number))
            },
        )
        .optional()?;
    match value_blob {
        None => Ok(None),
        Some((Some(value_blob), _)) => Ok(Some(words_from_blob(&value_blob))),
        Some((None, block_number)) => {
            super::finalized::query_state_inclusive_block(conn, contract_ca, key, block_number)
        }
    }
}

/// Query for the most recent version value of a key in a contracts state
/// that was set before the given block number.
///
/// This is exclusive of the block's state (..block_address).
pub fn query_state_exclusive_block(
    tx: &Transaction,
    contract_ca: &ContentAddress,
    key: &Key,
    block_address: &ContentAddress,
) -> Result<Option<Value>, QueryError> {
    let Some(parent_hash) = crate::get_parent_block_address(tx, block_address)? else {
        return Ok(None);
    };
    query_state_inclusive_block(tx, contract_ca, key, &parent_hash)
}

/// Query for the most recent version value of a key in a contracts state
/// that was set at or before the given block address and
/// solution set index (within that block).
///
/// This is inclusive of the solution's state mutations
/// `..=block_address[..=solution_set_index]`
pub fn query_state_inclusive_solution_set(
    conn: &Connection,
    contract_ca: &ContentAddress,
    key: &Key,
    block_address: &ContentAddress,
    solution_set_index: u64,
) -> Result<Option<Value>, QueryError> {
    let mut stmt = conn.prepare(sql::query::QUERY_STATE_BLOCK_ADDRESS)?;
    let value_blob: Option<(Option<Vec<u8>>, Word)> = stmt
        .query_row(
            named_params! {
                ":contract_ca": contract_ca.0,
                ":key": blob_from_words(key),
                ":block_address": block_address.0,
                ":solution_set_index": Some(solution_set_index),
            },
            |row| {
                let value = row.get("found_value")?;
                let number = row.get("number")?;
                Ok((value, number))
            },
        )
        .optional()?;
    match value_blob {
        None => Ok(None),
        Some((Some(value_blob), _)) => Ok(Some(words_from_blob(&value_blob))),
        Some((None, block_number)) => {
            super::finalized::query_state_inclusive_block(conn, contract_ca, key, block_number)
        }
    }
}

/// Query for the most recent version value of a key in a contracts state
/// that was set at or before the given block address and before the
/// solution set index (within that block).
///
/// This is exclusive of the solution set's state `..=block_address[..solution_set_index]`.
pub fn query_state_exclusive_solution_set(
    tx: &Transaction,
    contract_ca: &ContentAddress,
    key: &Key,
    block_address: &ContentAddress,
    solution_set_index: u64,
) -> Result<Option<Value>, QueryError> {
    match solution_set_index.checked_sub(1) {
        Some(solution_set_index) => query_state_inclusive_solution_set(
            tx,
            contract_ca,
            key,
            block_address,
            solution_set_index,
        ),
        None => query_state_exclusive_block(tx, contract_ca, key, block_address),
    }
}
