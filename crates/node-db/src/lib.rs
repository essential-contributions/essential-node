//! The node's DB interface and sqlite implementation.
//!
//! The core capability of the node is to receive blocks and their data from an
//! L1 relayer and derive state from these solutions.
//!
//! To satisfy these capabilities, the node requires the following DB interactions.
//!
//! 1. Write blocks and their data received from the relayer.
//! 2. Read blocks and their data to derive state.
//! 3. Write state.

pub use error::{DecodeError, QueryError};
use essential_hash::content_addr;
pub use essential_node_db_sql as sql;
use essential_types::{
    contract::Contract, predicate::Predicate, solution::Solution, Block, ContentAddress, Hash, Key,
    Value,
};
use rusqlite::{named_params, Connection};
use serde::{Deserialize, Serialize};
use std::{ops::Range, time::Duration};

mod error;

/// Encodes the given value into a hex-string blob.
///
/// Serializes the value using postcard before encoding the bytes as an uppercase hex-string.
pub fn encode<T>(value: &T) -> String
where
    T: Serialize,
{
    let value = postcard::to_allocvec(value).expect("postcard serialization cannot fail");
    hex::encode_upper(value)
}

/// Decodes the given hex-string blob into a value of type `T`.
///
/// Decodes the hex-string into bytes, before deserializing into a value of `T` with `postcard`.
pub fn decode<T>(value: &str) -> Result<T, DecodeError>
where
    T: for<'de> Deserialize<'de>,
{
    let value = hex::decode(value)?;
    Ok(postcard::from_bytes(&value)?)
}

/// Create all tables.
pub fn create_tables(conn: &Connection) -> rusqlite::Result<()> {
    for table in sql::table::ALL {
        conn.execute(table.create, ())?;
    }
    Ok(())
}

/// For the given block:
///
/// 1. Insert an entry into the blocks table.
/// 2. Insert each of its solutions into the solution and block_solution tables.
pub fn insert_block(conn: &Connection, block: &Block) -> rusqlite::Result<()> {
    // Insert the header.
    let secs = block.timestamp.as_secs();
    let nanos = block.timestamp.subsec_nanos() as u64;
    conn.execute(
        sql::insert::BLOCK,
        named_params! {
            ":number": block.number,
            ":created_at_seconds": secs,
            ":created_at_nanos": nanos,
        },
    )?;

    // Insert all solutions.
    for (ix, solution) in block.solutions.iter().enumerate() {
        let ca_blob = encode(&content_addr(solution));

        // Insert the solution.
        let solution_blob = encode(solution);
        conn.execute(
            sql::insert::SOLUTION,
            named_params! {
                ":content_hash": ca_blob,
                ":solution": solution_blob,
            },
        )?;

        // Create a mapping between the block and the solution.
        conn.execute(
            sql::insert::BLOCK_SOLUTION,
            named_params! {
                ":block_number": block.number,
                ":solution_hash": ca_blob,
                ":solution_index": ix,
            },
        )?;
    }

    Ok(())
}

/// For the given contract:
///
/// 1. Insert it into the contracts table.
/// 2. For each predicate, add entries to the predicate and contract_predicate tables.
pub fn insert_contract(
    conn: &Connection,
    contract: &Contract,
    // TODO: Provide the block header in which the contract first appeared
    // instead of the timestamp?
    timestamp: Duration,
) -> rusqlite::Result<()> {
    // Collect the predicate content addresses.
    let predicate_cas: Vec<_> = contract.predicates.iter().map(content_addr).collect();

    // Determine the contract's content address.
    let contract_ca = essential_hash::contract_addr::from_predicate_addrs(
        predicate_cas.iter().cloned(),
        &contract.salt,
    );

    // Encode the data into hex blobs.
    let contract_ca_blob = encode(&contract_ca);
    let salt_blob = encode(&contract.salt);
    let secs = timestamp.as_secs();
    let nanos = timestamp.subsec_nanos();
    conn.execute(
        sql::insert::CONTRACT,
        named_params! {
            ":content_hash": contract_ca_blob,
            ":salt": salt_blob,
            ":created_at_seconds": secs,
            ":created_at_nanos": nanos,
        },
    )?;

    // Insert the predicates and their pairings.
    for (pred, pred_ca) in contract.predicates.iter().zip(&predicate_cas) {
        let pred_blob = encode(pred);

        // Insert the predicate.
        let pred_ca_blob = encode(pred_ca);
        conn.execute(
            sql::insert::PREDICATE,
            named_params! {
                ":content_hash": &pred_ca_blob,
                ":predicate": pred_blob,
            },
        )?;

        // Insert the pairing.
        conn.execute(
            sql::insert::CONTRACT_PREDICATE,
            named_params! {
                ":contract_hash": &contract_ca_blob,
                ":predicate_hash": &pred_ca_blob,
            },
        )?;
    }

    Ok(())
}

/// Updates the state for a given contract content hash and key.
pub fn update_state(
    conn: &Connection,
    contract_ca: &ContentAddress,
    key: &Key,
    value: &Value,
) -> rusqlite::Result<()> {
    let contract_ca_blob = encode(contract_ca);
    let key_blob = encode(key);
    let value_blob = encode(value);
    conn.execute(
        sql::update::STATE,
        named_params! {
            ":contract_hash": contract_ca_blob,
            ":key": key_blob,
            ":value": value_blob,
        },
    )?;
    Ok(())
}

/// Deletes the state for a given contract content hash and key.
pub fn delete_state(
    conn: &Connection,
    contract_ca: &ContentAddress,
    key: &Key,
) -> rusqlite::Result<()> {
    let contract_ca_blob = encode(contract_ca);
    let key_blob = encode(key);
    conn.execute(
        sql::update::DELETE_STATE,
        named_params! {
            ":contract_hash": contract_ca_blob,
            ":key": key_blob,
        },
    )?;
    Ok(())
}

/// Fetches a contract's salt by its content address.
pub fn get_contract_salt(
    conn: &Connection,
    ca: &ContentAddress,
) -> Result<Option<Hash>, QueryError> {
    let ca_blob = encode(ca);
    get_contract_salt_by_ca_blob(conn, &ca_blob)
}

/// Fetches a contract's predicates by its content address.
pub fn get_contract_predicates(
    conn: &Connection,
    ca: &ContentAddress,
) -> Result<Option<Vec<Predicate>>, QueryError> {
    let ca_blob = encode(ca);
    get_contract_predicates_by_ca_blob(conn, &ca_blob)
}

/// Fetches a contract by its content address.
pub fn get_contract(
    conn: &Connection,
    ca: &ContentAddress,
) -> Result<Option<Contract>, QueryError> {
    let ca_blob = encode(ca);
    get_contract_by_ca_blob(conn, &ca_blob)
}

/// Fetches a predicate by its content hash.
pub fn get_predicate(
    conn: &Connection,
    ca: &ContentAddress,
) -> Result<Option<Predicate>, QueryError> {
    let ca_blob = encode(ca);
    get_predicate_by_ca_blob(conn, &ca_blob)
}

/// Fetches a solution by its content hash.
pub fn get_solution(conn: &Connection, ca: &ContentAddress) -> Result<Option<Vec<u8>>, QueryError> {
    let ca_blob = encode(ca);
    get_solution_by_ca_blob(conn, &ca_blob)
}

/// Fetches the state value by contract content hash and key.
pub fn get_state_value(
    conn: &Connection,
    contract_ca: &ContentAddress,
    key: &Key,
) -> Result<Option<Vec<u8>>, QueryError> {
    let contract_ca_blob = encode(contract_ca);
    let key_blob = encode(key);
    get_state_value_by_blobs(conn, &contract_ca_blob, &key_blob)
}

/// Lists all blocks in the given range.
pub fn list_blocks(conn: &Connection, block_range: Range<u64>) -> Result<Vec<Block>, QueryError> {
    let mut stmt = conn.prepare(sql::query::LIST_BLOCKS)?;
    let mut rows = stmt.query_map(
        named_params! {
            ":start_block": block_range.start,
            ":end_block": block_range.end,
        },
        |row| {
            const BLOCK_NUMBER: usize = 0;
            const CREATED_AT_SECONDS: usize = 1;
            const CREATED_AT_NANOS: usize = 2;
            const SOLUTION: usize = 3;
            let block_number: u64 = row.get(BLOCK_NUMBER)?;
            let created_at_seconds: u64 = row.get(CREATED_AT_SECONDS)?;
            let created_at_nanos: u32 = row.get(CREATED_AT_NANOS)?;
            let solution_blob: String = row.get(SOLUTION)?;
            let timestamp = Duration::new(created_at_seconds, created_at_nanos);
            Ok((block_number, timestamp, solution_blob))
        },
    )?;

    // Query yields in order of block number and solution index.
    let mut blocks: Vec<Block> = vec![];
    let mut last_block_number = None;
    while let Some(res) = rows.next() {
        let (block_number, timestamp, solution_blob): (u64, Duration, String) = res?;

        // Fetch the block associated with the block number, inserting it first if new.
        let block = match last_block_number {
            Some(n) if n == block_number => blocks.last_mut().expect("last block must exist"),
            _ => {
                last_block_number = Some(block_number);
                blocks.push(Block {
                    number: block_number,
                    timestamp,
                    solutions: vec![],
                });
                blocks.last_mut().unwrap()
            }
        };

        // Add the solution.
        let solution: Solution = decode(&solution_blob)?;
        block.solutions.push(solution);
    }
    Ok(blocks)
}

/// Lists blocks and their solutions within a specific time range with pagination.
pub fn list_blocks_by_time(
    conn: &Connection,
    start: Duration,
    end: Duration,
    page_size: i64,
    page_number: i64,
) -> Result<Vec<Block>, QueryError> {
    const BLOCK_NUMBER: usize = 0;
    const CREATED_AT_SECONDS: usize = 1;
    const CREATED_AT_NANOS: usize = 2;
    const SOLUTION: usize = 3;
    let mut stmt = conn.prepare(sql::query::LIST_BLOCKS_BY_TIME)?;
    let mut rows = stmt.query_map(
        named_params! {
            ":start_seconds": start.as_secs(),
            ":start_nanos": start.subsec_nanos(),
            ":end_seconds": end.as_secs(),
            ":end_nanos": end.subsec_nanos(),
            ":page_size": page_size,
            ":page_number": page_number,
        },
        |row| {
            let block_number: u64 = row.get(BLOCK_NUMBER)?;
            let created_at_seconds: u64 = row.get(CREATED_AT_SECONDS)?;
            let created_at_nanos: u32 = row.get(CREATED_AT_NANOS)?;
            let solution_blob: String = row.get(SOLUTION)?;
            let timestamp = Duration::new(created_at_seconds, created_at_nanos);
            Ok((block_number, timestamp, solution_blob))
        },
    )?;

    // Query yields in order of block number and solution index.
    let mut blocks: Vec<Block> = vec![];
    let mut last_block_number: Option<u64> = None;
    while let Some(res) = rows.next() {
        let (block_number, timestamp, solution_blob): (u64, Duration, String) = res?;

        // Fetch the block associated with the block number, inserting it first if new.
        let block = match last_block_number {
            Some(n) if n == block_number => blocks.last_mut().expect("last block must exist"),
            _ => {
                last_block_number = Some(block_number);
                blocks.push(Block {
                    number: block_number,
                    timestamp,
                    solutions: vec![],
                });
                blocks.last_mut().unwrap()
            }
        };

        // Add the solution.
        let solution: Solution = decode(&solution_blob)?;
        block.solutions.push(solution);
    }
    Ok(blocks)
}

// Helper Functions

fn get_contract_salt_by_ca_blob(
    conn: &Connection,
    ca_blob: &str,
) -> Result<Option<Hash>, QueryError> {
    const SALT: usize = 0;
    let mut stmt = conn.prepare(sql::query::GET_CONTRACT_SALT)?;
    let salt_blob: String = stmt.query_row([ca_blob], |row| row.get(SALT))?;
    let hash = decode(&salt_blob)?;
    Ok(hash)
}

fn get_contract_predicates_by_ca_blob(
    conn: &Connection,
    ca_blob: &str,
) -> Result<Option<Vec<Predicate>>, QueryError> {
    let mut stmt = conn.prepare(sql::query::GET_CONTRACT)?;
    const PREDICATE: usize = 0;
    let mut pred_blobs = stmt.query_map([ca_blob], |row| row.get::<_, String>(PREDICATE))?;
    let mut predicates: Vec<Predicate> = vec![];
    while let Some(pred_blob) = pred_blobs.next() {
        predicates.push(decode(&pred_blob?)?);
    }
    Ok(Some(predicates))
}

fn get_contract_by_ca_blob(
    conn: &Connection,
    ca_blob: &str,
) -> Result<Option<Contract>, QueryError> {
    let Some(salt) = get_contract_salt_by_ca_blob(conn, ca_blob)? else {
        return Ok(None);
    };
    let Some(predicates) = get_contract_predicates_by_ca_blob(conn, ca_blob)? else {
        return Ok(None);
    };
    Ok(Some(Contract { salt, predicates }))
}

fn get_predicate_by_ca_blob(
    conn: &Connection,
    ca_blob: &str,
) -> Result<Option<Predicate>, QueryError> {
    const PREDICATE: usize = 0;
    let mut stmt = conn.prepare(sql::query::GET_PREDICATE)?;
    let pred_blob: String = stmt.query_row([ca_blob], |row| row.get(PREDICATE))?;
    let predicate = decode(&pred_blob)?;
    Ok(predicate)
}

fn get_solution_by_ca_blob(
    conn: &Connection,
    ca_blob: &str,
) -> Result<Option<Vec<u8>>, QueryError> {
    const SOLUTION: usize = 0;
    let mut stmt = conn.prepare(sql::query::GET_SOLUTION)?;
    let solution_blob: String = stmt.query_row([ca_blob], |row| row.get(SOLUTION))?;
    let solution = decode(&solution_blob)?;
    Ok(solution)
}

fn get_state_value_by_blobs(
    conn: &Connection,
    contract_ca_blob: &str,
    key_blob: &str,
) -> Result<Option<Vec<u8>>, QueryError> {
    const VALUE: usize = 0;
    let mut stmt = conn.prepare(sql::query::GET_STATE)?;
    let value_blob: String = stmt.query_row([contract_ca_blob, key_blob], |row| row.get(VALUE))?;
    let value = decode(&value_blob)?;
    Ok(value)
}
