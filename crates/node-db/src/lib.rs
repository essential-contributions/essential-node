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
    PredicateAddress, Value,
};
use rusqlite::{named_params, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use std::{ops::Range, time::Duration};

mod error;

/// Encodes the given value into a blob.
///
/// This serializes the value using postcard.
pub fn encode<T>(value: &T) -> Vec<u8>
where
    T: Serialize,
{
    postcard::to_allocvec(value).expect("postcard serialization cannot fail")
}

/// Decodes the given blob into a value of type `T`.
///
/// This deserializes the bytes into a value of `T` with `postcard`.
pub fn decode<T>(value: &[u8]) -> Result<T, DecodeError>
where
    T: for<'de> Deserialize<'de>,
{
    Ok(postcard::from_bytes(value)?)
}

/// Create all tables.
pub fn create_tables(tx: &Transaction) -> rusqlite::Result<()> {
    for table in sql::table::ALL {
        tx.execute(table.create, ())?;
    }
    Ok(())
}

/// For the given block:
///
/// 1. Insert an entry into the blocks table.
/// 2. Insert each of its solutions into the solution and block_solution tables.
pub fn insert_block(tx: &Transaction, block: &Block) -> rusqlite::Result<()> {
    // Insert the header.
    let secs = block.timestamp.as_secs();
    let nanos = block.timestamp.subsec_nanos() as u64;
    tx.execute(
        sql::insert::BLOCK,
        named_params! {
            ":number": block.number,
            ":timestamp_secs": secs,
            ":timestamp_nanos": nanos,
        },
    )?;

    // Insert all solutions.
    for (ix, solution) in block.solutions.iter().enumerate() {
        let ca_blob = encode(&content_addr(solution));

        // Insert the solution.
        let solution_blob = encode(solution);
        tx.execute(
            sql::insert::SOLUTION,
            named_params! {
                ":content_hash": ca_blob,
                ":solution": solution_blob,
            },
        )?;

        // Create a mapping between the block and the solution.
        tx.execute(
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
    tx: &Transaction,
    contract: &Contract,
    da_block_number: u64,
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
    tx.execute(
        sql::insert::CONTRACT,
        named_params! {
            ":content_hash": contract_ca_blob,
            ":salt": salt_blob,
            ":da_block_number": da_block_number,
        },
    )?;

    // Insert the predicates and their pairings.
    for (pred, pred_ca) in contract.predicates.iter().zip(&predicate_cas) {
        let pred_blob = encode(pred);

        // Insert the predicate.
        let pred_ca_blob = encode(pred_ca);
        tx.execute(
            sql::insert::PREDICATE,
            named_params! {
                ":content_hash": &pred_ca_blob,
                ":predicate": pred_blob,
            },
        )?;

        // Insert the pairing.
        tx.execute(
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

fn get_contract_salt_by_ca_blob(
    conn: &Connection,
    ca_blob: &[u8],
) -> Result<Option<Hash>, QueryError> {
    let mut stmt = conn.prepare(sql::query::GET_CONTRACT_SALT)?;
    let salt_blob: Option<Vec<u8>> = stmt
        .query_row([ca_blob], |row| row.get("salt"))
        .optional()?;
    Ok(salt_blob.as_deref().map(decode).transpose()?)
}

/// Fetches a contract's predicates by its content address.
pub fn get_contract_predicates(
    conn: &Connection,
    ca: &ContentAddress,
) -> Result<Option<Vec<Predicate>>, QueryError> {
    let ca_blob = encode(ca);
    get_contract_predicates_by_ca_blob(conn, &ca_blob)
}

fn get_contract_predicates_by_ca_blob(
    conn: &Connection,
    ca_blob: &[u8],
) -> Result<Option<Vec<Predicate>>, QueryError> {
    let mut stmt = conn.prepare(sql::query::GET_CONTRACT_PREDICATES)?;
    let pred_blobs = stmt.query_map([ca_blob], |row| row.get::<_, Vec<u8>>("predicate"))?;
    let mut predicates: Vec<Predicate> = vec![];
    for pred_blob in pred_blobs {
        predicates.push(decode(&pred_blob?)?);
    }
    Ok(Some(predicates))
}

/// Fetches a contract by its content address.
pub fn get_contract(
    conn: &Connection,
    ca: &ContentAddress,
) -> Result<Option<Contract>, QueryError> {
    let ca_blob = encode(ca);
    let Some(salt) = get_contract_salt_by_ca_blob(conn, &ca_blob)? else {
        return Ok(None);
    };
    let Some(predicates) = get_contract_predicates_by_ca_blob(conn, &ca_blob)? else {
        return Ok(None);
    };
    Ok(Some(Contract { salt, predicates }))
}

/// Fetches a predicate by its content hash.
pub fn get_predicate(
    conn: &Connection,
    addr: &PredicateAddress,
) -> Result<Option<Predicate>, QueryError> {
    let contract_ca_blob = encode(&addr.contract);
    let predicate_ca_blob = encode(&addr.predicate);
    let mut stmt = conn.prepare(sql::query::GET_PREDICATE)?;
    let pred_blob: Option<Vec<u8>> = stmt
        .query_row(
            named_params! {
                ":contract_hash": contract_ca_blob,
                ":predicate_hash": predicate_ca_blob,
            },
            |row| row.get("predicate"),
        )
        .optional()?;
    Ok(pred_blob.as_deref().map(decode).transpose()?)
}

/// Fetches a solution by its content hash.
pub fn get_solution(
    conn: &Connection,
    ca: &ContentAddress,
) -> Result<Option<Solution>, QueryError> {
    let ca_blob = encode(ca);
    let mut stmt = conn.prepare(sql::query::GET_SOLUTION)?;
    let solution_blob: Option<Vec<u8>> = stmt
        .query_row([ca_blob], |row| row.get("solution"))
        .optional()?;
    Ok(solution_blob.as_deref().map(decode).transpose()?)
}

/// Fetches the state value by contract content hash and key.
pub fn get_state_value(
    conn: &Connection,
    contract_ca: &ContentAddress,
    key: &Key,
) -> Result<Option<Value>, QueryError> {
    use rusqlite::OptionalExtension;
    let contract_ca_blob = encode(contract_ca);
    let key_blob = encode(key);
    let mut stmt = conn.prepare(sql::query::GET_STATE)?;
    let value_blob: Option<Vec<u8>> = stmt
        .query_row([contract_ca_blob, key_blob], |row| row.get("value"))
        .optional()?;
    Ok(value_blob.as_deref().map(decode).transpose()?)
}

/// Lists all blocks in the given range.
pub fn list_blocks(conn: &Connection, block_range: Range<u64>) -> Result<Vec<Block>, QueryError> {
    let mut stmt = conn.prepare(sql::query::LIST_BLOCKS)?;
    let rows = stmt.query_map(
        named_params! {
            ":start_block": block_range.start,
            ":end_block": block_range.end,
        },
        |row| {
            let block_number: u64 = row.get("number")?;
            let timestamp_secs: u64 = row.get("timestamp_secs")?;
            let timestamp_nanos: u32 = row.get("timestamp_nanos")?;
            let solution_blob: Vec<u8> = row.get("solution")?;
            let timestamp = Duration::new(timestamp_secs, timestamp_nanos);
            Ok((block_number, timestamp, solution_blob))
        },
    )?;

    // Query yields in order of block number and solution index.
    let mut blocks: Vec<Block> = vec![];
    let mut last_block_number = None;
    for res in rows {
        let (block_number, timestamp, solution_blob): (u64, Duration, Vec<u8>) = res?;

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
    range: Range<Duration>,
    page_size: i64,
    page_number: i64,
) -> Result<Vec<Block>, QueryError> {
    let mut stmt = conn.prepare(sql::query::LIST_BLOCKS_BY_TIME)?;
    let rows = stmt.query_map(
        named_params! {
            ":start_secs": range.start.as_secs(),
            ":start_nanos": range.start.subsec_nanos(),
            ":end_secs": range.end.as_secs(),
            ":end_nanos": range.end.subsec_nanos(),
            ":page_size": page_size,
            ":page_number": page_number,
        },
        |row| {
            let block_number: u64 = row.get("number")?;
            let timestamp_secs: u64 = row.get("timestamp_secs")?;
            let timestamp_nanos: u32 = row.get("timestamp_nanos")?;
            let solution_blob: Vec<u8> = row.get("solution")?;
            let timestamp = Duration::new(timestamp_secs, timestamp_nanos);
            Ok((block_number, timestamp, solution_blob))
        },
    )?;

    // Query yields in order of block number and solution index.
    let mut blocks: Vec<Block> = vec![];
    let mut last_block_number: Option<u64> = None;
    for res in rows {
        let (block_number, timestamp, solution_blob): (u64, Duration, Vec<u8>) = res?;

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

/// Lists contracts and their predicates within a given DA block range.
///
/// Returns each non-empty DA block number in the range alongside a
/// `Vec<Contract>` containing the contracts appearing in that block.
pub fn list_contracts(
    conn: &Connection,
    block_range: Range<u64>,
) -> Result<Vec<(u64, Vec<Contract>)>, QueryError> {
    let mut stmt = conn.prepare(sql::query::LIST_CONTRACTS)?;
    let rows = stmt.query_map(
        named_params! {
            ":start_block": block_range.start,
            ":end_block": block_range.end,
        },
        |row| {
            let block_num: u64 = row.get("da_block_number")?;
            let salt_blob: Vec<u8> = row.get("salt")?;
            let contract_ca_blob: Vec<u8> = row.get("content_hash")?;
            let pred_blob: Vec<u8> = row.get("predicate")?;
            Ok((block_num, contract_ca_blob, salt_blob, pred_blob))
        },
    )?;

    // Query yields in order of block number and predicate ID.
    let mut blocks: Vec<(u64, Vec<Contract>)> = vec![];
    let mut last_block_num: Option<u64> = None;
    let mut last_contract_ca = None;
    for res in rows {
        let (da_block_num, ca_blob, salt_blob, pred_blob): (u64, Vec<u8>, Vec<u8>, Vec<u8>) = res?;
        let contract_ca: ContentAddress = decode(&ca_blob)?;
        let salt: Hash = decode(&salt_blob)?;

        // Fetch the block entry associated with the given block number or insert if new.
        let block = match last_block_num {
            Some(n) if n == da_block_num => blocks.last_mut().expect("block entry must exist"),
            _ => {
                last_block_num = Some(da_block_num);
                last_contract_ca = None;
                blocks.push((da_block_num, vec![]));
                blocks.last_mut().expect("block entry must exist")
            }
        };

        // Fetch the contract associated with the CA or insert if new.
        let contract = match last_contract_ca {
            Some(ref ca) if ca == &contract_ca => block.1.last_mut().expect("entry must exist"),
            _ => {
                last_contract_ca = Some(contract_ca);
                let predicates = vec![];
                block.1.push(Contract { salt, predicates });
                block.1.last_mut().expect("entry must exist")
            }
        };

        let pred: Predicate = decode(&pred_blob)?;
        contract.predicates.push(pred);
    }

    Ok(blocks)
}
