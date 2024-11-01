//! Finalized queries query for the most recent version of a key less than or equal to a
//! given block number or solution index for blocks that have been finalized.

use crate::{blob_from_words, words_from_blob, QueryError};
use essential_node_db_sql as sql;
use essential_types::{ContentAddress, Key, Value, Word};
use rusqlite::{named_params, Connection, OptionalExtension};

/// Query the most recent value for a key in a contract's state
/// that was set at or before the given block number.
///
/// This is inclusive of the block's state (..=block_number).
pub fn query_state_inclusive_block(
    conn: &Connection,
    contract_ca: &ContentAddress,
    key: &Key,
    block_number: Word,
) -> Result<Option<Value>, QueryError> {
    let mut stmt = conn.prepare(sql::query::QUERY_STATE_AT_BLOCK_FINALIZED)?;
    let value_blob: Option<Vec<u8>> = stmt
        .query_row(
            named_params! {
                ":contract_ca": contract_ca.0,
                ":key": blob_from_words(key),
                ":block_number": block_number,
            },
            |row| row.get("value"),
        )
        .optional()?;
    Ok(value_blob.as_deref().map(words_from_blob))
}

/// Query for the most recent version value of a key in a contracts state
/// that was set before the given block number.
///
/// This is exclusive of the block's state (..block_number).
pub fn query_state_exclusive_block(
    conn: &Connection,
    contract_ca: &ContentAddress,
    key: &Key,
    block_number: Word,
) -> Result<Option<Value>, QueryError> {
    match block_number.checked_sub(1) {
        Some(block_number) => query_state_inclusive_block(conn, contract_ca, key, block_number),
        None => Ok(None),
    }
}

/// Query for the most recent version value of a key in a contracts state
/// that was set at or before the given block number and
/// solution index (within that block).
///
/// This is inclusive of the solution's state mutations
/// `..=block_number[..=solution_index]`
pub fn query_state_inclusive_solution(
    conn: &Connection,
    contract_ca: &ContentAddress,
    key: &Key,
    block_number: Word,
    solution_index: u64,
) -> Result<Option<Value>, QueryError> {
    let mut stmt = conn.prepare(sql::query::QUERY_STATE_AT_SOLUTION_FINALIZED)?;
    let value_blob: Option<Vec<u8>> = stmt
        .query_row(
            named_params! {
                ":contract_ca": contract_ca.0,
                ":key": blob_from_words(key),
                ":block_number": block_number,
                ":solution_index": solution_index,
            },
            |row| row.get("value"),
        )
        .optional()?;
    Ok(value_blob.as_deref().map(words_from_blob))
}

/// Query for the most recent version value of a key in a contracts state
/// that was set at or before the given block number and before the
/// solution index (within that block).
///
/// This is exclusive of the solution's state `..=block_number[..solution_index]`.
pub fn query_state_exclusive_solution(
    conn: &Connection,
    contract_ca: &ContentAddress,
    key: &Key,
    block_number: Word,
    solution_index: u64,
) -> Result<Option<Value>, QueryError> {
    match solution_index.checked_sub(1) {
        Some(solution_index) => {
            query_state_inclusive_solution(conn, contract_ca, key, block_number, solution_index)
        }
        None => query_state_exclusive_block(conn, contract_ca, key, block_number),
    }
}
