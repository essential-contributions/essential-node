//! Finalized queries query for the most recent version of a key less than or equal to a
//! given block number or solution index for blocks that have been finalized.

use essential_node_db_sql as sql;
use essential_types::{ContentAddress, Key, Value};
use rusqlite::{named_params, OptionalExtension, Transaction};

use crate::{decode, encode, QueryError};

/// Query for the most recent version value of a key in a contracts state
/// that was set at or before the given block number.
///
/// This is inclusive of the block's state (..=block_number).
pub fn query_state_inclusive_block(
    tx: &Transaction,
    contract_ca: &ContentAddress,
    key: &Key,
    block_number: u64,
) -> Result<Option<Value>, QueryError> {
    let contract_ca_blob = encode(contract_ca);
    let key_blob = encode(key);
    let mut stmt = tx.prepare(sql::query::QUERY_STATE_AT_BLOCK_FINALIZED)?;
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
/// that was set before the given block number.
///
/// This is exclusive of the block's state (..block_number).
pub fn query_state_exclusive_block(
    tx: &Transaction,
    contract_ca: &ContentAddress,
    key: &Key,
    block_number: u64,
) -> Result<Option<Value>, QueryError> {
    match block_number.checked_sub(1) {
        Some(block_number) => query_state_inclusive_block(tx, contract_ca, key, block_number),
        None => Ok(None),
    }
}

/// Query for the most recent version value of a key in a contracts state
/// that was set at or before the given block number and
/// solution index (within that block).
///
/// This is inclusive of the block's state and inclusive of the solution's state
/// (..=(block_number, solution_index)).
pub fn query_state_inclusive_solution(
    tx: &Transaction,
    contract_ca: &ContentAddress,
    key: &Key,
    block_number: u64,
    solution_index: u64,
) -> Result<Option<Value>, QueryError> {
    let contract_ca_blob = encode(contract_ca);
    let key_blob = encode(key);
    let mut stmt = tx.prepare(sql::query::QUERY_STATE_AT_SOLUTION_FINALIZED)?;
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

/// Query for the most recent version value of a key in a contracts state
/// that was set at or before the given block number and before the
/// solution index (within that block).
///
/// This is exclusive of the solution's state (..(block_number, solution_index)).
pub fn query_state_exclusive_solution(
    tx: &Transaction,
    contract_ca: &ContentAddress,
    key: &Key,
    block_number: u64,
    solution_index: u64,
) -> Result<Option<Value>, QueryError> {
    match solution_index.checked_sub(1) {
        Some(solution_index) => {
            query_state_inclusive_solution(tx, contract_ca, key, block_number, solution_index)
        }
        None => query_state_exclusive_block(tx, contract_ca, key, block_number),
    }
}
