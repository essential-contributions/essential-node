//! Multiverse queries query for the most recent version of a key less than or equal to a
//! given block number or solution index across all histories.
//!
//! This means that you do not specify the history (branch) you want to query, and the query
//! you will not know which history the value came from.
//!
//! This is useful when you know for sure that there is only a single history.
//! It is not recommended to use these queries when there are multiple histories.

use essential_node_db_sql as sql;
use essential_types::{ContentAddress, Key, Value};
use rusqlite::{named_params, OptionalExtension, Transaction};

use crate::{decode, encode, QueryError};

/// Query for the most recent version value of a key in a contracts state
/// that was set at or before the given block number.
///
/// This is inclusive of the block's state.
pub fn query_state_at_block(
    tx: &Transaction,
    contract_ca: &ContentAddress,
    key: &Key,
    block_number: u64,
) -> Result<Option<Value>, QueryError> {
    let contract_ca_blob = encode(contract_ca);
    let key_blob = encode(key);
    let mut stmt = tx.prepare(sql::query::QUERY_STATE_AT_BLOCK_MULTIVERSE)?;
    let value_blob: Option<Vec<u8>> = stmt
        .query_row(
            named_params! {
                ":contract_ca": contract_ca_blob,
                ":key": key_blob,
                ":block_number": block_number,
            },
            |row| row.get("value"),
        )
        .optional()?;
    Ok(value_blob.as_deref().map(decode).transpose()?)
}

/// Query for the most recent version value of a key in a contracts state
/// that was set at or before the given block number and
/// solution index (within that block).
///
/// This is inclusive of the block's state and inclusive of the solution's state.
pub fn query_state_at_solution(
    tx: &Transaction,
    contract_ca: &ContentAddress,
    key: &Key,
    block_number: u64,
    solution_index: u64,
) -> Result<Option<Value>, QueryError> {
    let contract_ca_blob = encode(contract_ca);
    let key_blob = encode(key);
    let mut stmt = tx.prepare(sql::query::QUERY_STATE_AT_SOLUTION_MULTIVERSE)?;
    let value_blob: Option<Vec<u8>> = stmt
        .query_row(
            named_params! {
                ":contract_ca": contract_ca_blob,
                ":key": key_blob,
                ":block_number": block_number,
                ":solution_index": solution_index,
            },
            |row| row.get("value"),
        )
        .optional()?;
    Ok(value_blob.as_deref().map(decode).transpose()?)
}
