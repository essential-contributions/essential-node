#![warn(missing_docs)]

//! The node's DB interface and sqlite implementation.
//!
//! The core capability of the node is to:
//!
//! 1. Receive blocks from an L1 relayer and derive state from their solutions.
//! 2. Receive contracts from the p2p network so that they're available for validation.
//!
//! As a part of satisfying these requirements, this crate provides the basic
//! functions required for safely creating the necessary tables and inserting/
//! querying/updating them as necessary.

pub use error::{DecodeError, QueryError};
use essential_hash::content_addr;
#[doc(inline)]
pub use essential_node_db_sql as sql;
use essential_types::{
    contract::Contract, predicate::Predicate, solution::Solution, Block, ContentAddress, Hash, Key,
    Value,
};
use rusqlite::{named_params, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use std::{ops::Range, time::Duration};

mod error;

#[non_exhaustive]
/// Position in the log of state mutations.
/// This type describes which block and solution you
/// want to view state at.
///
/// This type will be expanded in the future to include
/// more ways of pointing to a specific position in the log.
pub enum StatePosition {
    /// Optimistic chain position.
    Optimistic(Optimistic),
}

/// The optimistic position is the first block
/// proposed for a given block number that this
/// node has not seen any evidence of fraud for.
pub struct Optimistic {
    /// The block number to view state as of.
    pub number: BlockNumber,
    /// The solution index withing the block.
    pub position: SolutionIndex,
}

/// The block number to view state as of.
pub enum BlockNumber {
    /// The latest block this node has seen.
    Latest,
    /// A specific block number.
    Number(u64),
}

/// The solution index within the block.
///
/// Note that because keys cannot be mutated
/// more than once in a solution, we cannot
/// get any more granular than the solution index.
pub enum SolutionIndex {
    /// The state as of the start of the block.
    /// Note this is equivalent to the state
    /// as of the end of the previous block.
    Start,
    /// The state as of the end of the block.
    End,
    /// The state as of the solution at the given index.
    /// This is the state after the solution has been applied.
    Index(u64),
}

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
/// 1. Insert an entry into the `block` table.
/// 2. Insert each of its solutions into the `solution` and `block_solution` tables.
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
    let mut stmt_solution = tx.prepare(sql::insert::SOLUTION)?;
    let mut stmt_block_solution = tx.prepare(sql::insert::BLOCK_SOLUTION)?;
    let mut stmt_mutation = tx.prepare(sql::insert::MUTATION)?;
    for (ix, solution) in block.solutions.iter().enumerate() {
        let ca_blob = encode(&content_addr(solution));

        // Insert the solution.
        let solution_blob = encode(solution);
        stmt_solution.execute(named_params! {
            ":content_hash": ca_blob,
            ":solution": solution_blob,
        })?;

        // Create a mapping between the block and the solution.
        stmt_block_solution.execute(named_params! {
            ":block_number": block.number,
            ":solution_hash": &ca_blob,
            ":solution_index": ix,
        })?;

        for (data_ix, data) in solution.data.iter().enumerate() {
            let contract_ca_blob = encode(&data.predicate_to_solve.contract);
            for (mutation_ix, mutation) in data.state_mutations.iter().enumerate() {
                let key_blob = encode(&mutation.key);
                let value_blob = encode(&mutation.value);
                stmt_mutation.execute(named_params! {
                    ":solution_hash": ca_blob,
                    ":data_index": data_ix,
                    ":mutation_index": mutation_ix,
                    ":contract_ca": contract_ca_blob,
                    ":key": key_blob,
                    ":value": value_blob,
                })?;
            }
        }
    }
    stmt_solution.finalize()?;
    stmt_block_solution.finalize()?;
    stmt_mutation.finalize()?;

    Ok(())
}

/// For the given contract:
///
/// 1. Insert it into the `contract` table.
/// 2. For each predicate, add entries to the `predicate` and `contract_predicate` tables.
///
/// The `l2_block_number` is the block containing the contract's DA proof.
pub fn insert_contract(
    tx: &Transaction,
    contract: &Contract,
    l2_block_number: u64,
) -> rusqlite::Result<()> {
    // Collect the predicate content addresses.
    let predicate_cas: Vec<_> = contract.predicates.iter().map(content_addr).collect();

    // Determine the contract's content address.
    let contract_ca = essential_hash::contract_addr::from_predicate_addrs(
        predicate_cas.iter().cloned(),
        &contract.salt,
    );

    // Encode the data into blobs.
    let contract_ca_blob = encode(&contract_ca);
    let salt_blob = encode(&contract.salt);
    tx.execute(
        sql::insert::CONTRACT,
        named_params! {
            ":content_hash": contract_ca_blob,
            ":salt": salt_blob,
            ":l2_block_number": l2_block_number,
        },
    )?;

    // Insert the predicates and their pairings.
    let mut stmt_predicate = tx.prepare(sql::insert::PREDICATE)?;
    let mut stmt_contract_predicate = tx.prepare(sql::insert::CONTRACT_PREDICATE)?;
    for (pred, pred_ca) in contract.predicates.iter().zip(&predicate_cas) {
        let pred_blob = encode(pred);

        // Insert the predicate.
        let pred_ca_blob = encode(pred_ca);
        stmt_predicate.execute(named_params! {
            ":content_hash": &pred_ca_blob,
            ":predicate": pred_blob,
        })?;

        // Insert the pairing.
        stmt_contract_predicate.execute(named_params! {
            ":contract_hash": &contract_ca_blob,
            ":predicate_hash": &pred_ca_blob,
        })?;
    }

    Ok(())
}

/// Inserts the new progress for syncing contracts.
///
/// Note even though this is an insert,
/// it will overwrite the previous progress.
/// This is because we need to initialize the first progress.
pub fn insert_contract_progress(
    conn: &Connection,
    l2_block_number: u64,
    contract_ca: &ContentAddress,
) -> rusqlite::Result<()> {
    let contract_ca_blob = encode(contract_ca);

    // Cast to i64 to match the type of the column.
    let l2_block_number = l2_block_number as i64;
    conn.execute(
        sql::insert::CONTRACT_PROGRESS,
        named_params! {
            ":l2_block_number": l2_block_number,
            ":content_hash": contract_ca_blob,
        },
    )?;
    Ok(())
}

/// Updates the state for a given contract content address and key.
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

/// Updates the progress on state derivation.
pub fn update_state_progress(
    conn: &Connection,
    block_number: u64,
    block_hash: &ContentAddress,
) -> rusqlite::Result<()> {
    let block_hash_blob = encode(block_hash);
    let block_number = block_number as i64;
    conn.execute(
        sql::insert::STATE_PROGRESS,
        named_params! {
            ":number": block_number,
            ":block_hash": block_hash_blob,
        },
    )?;
    Ok(())
}

/// Deletes the state for a given contract content address and key.
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

/// Fetches the salt of the contract with the given content address.
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

/// Fetches the predicates of the contract with the given content address.
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

/// Fetches the contract progress.
pub fn get_contract_progress(
    conn: &Connection,
) -> Result<Option<(u64, ContentAddress)>, QueryError> {
    let Some((l2_block_number, content_hash)) = conn
        .query_row(sql::query::GET_CONTRACT_PROGRESS, [], |row| {
            let l2_block_number: i64 = row.get("l2_block_number")?;
            let content_hash: Vec<u8> = row.get("content_hash")?;
            Ok((l2_block_number as u64, content_hash))
        })
        .optional()?
    else {
        return Ok(None);
    };
    let content_hash = decode(&content_hash)?;
    Ok(Some((l2_block_number, content_hash)))
}

/// Fetches a predicate by its predicate content address.
pub fn get_predicate(
    conn: &Connection,
    predicate_ca: &ContentAddress,
) -> Result<Option<Predicate>, QueryError> {
    let predicate_ca_blob = encode(predicate_ca);
    let mut stmt = conn.prepare(sql::query::GET_PREDICATE)?;
    let pred_blob: Option<Vec<u8>> = stmt
        .query_row(
            named_params! {
                ":predicate_hash": predicate_ca_blob,
            },
            |row| row.get("predicate"),
        )
        .optional()?;
    Ok(pred_blob.as_deref().map(decode).transpose()?)
}

/// Fetches a solution by its content address.
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

/// Fetches the state value for the given contract content address and key pair.
pub fn query_state(
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

/// Query state at a specific position.
pub fn query_state_at(
    tx: &Transaction,
    contract_ca: &ContentAddress,
    key: &Key,
    position: &StatePosition,
) -> Result<Option<Value>, QueryError> {
    let Some(solution_ptr) = state_position_to_solution_ptr(tx, position)? else {
        return Ok(None);
    };
    let contract_ca_blob = encode(contract_ca);
    let key_blob = encode(key);
    let mut stmt = tx.prepare(sql::query::QUERY_STATE_AT_OPTIMISTIC)?;
    let value_blob: Option<Vec<u8>> = stmt
        .query_row(
            named_params! {
                ":contract_ca": contract_ca_blob,
                ":key": key_blob,
                ":block_number": solution_ptr.block_number,
                ":solution_index": solution_ptr.solution_index,
            },
            |row| row.get("value"),
        )
        .optional()?;
    Ok(value_blob.as_deref().map(decode).transpose()?)
}

/// Fetches the latest block number.
pub fn get_latest_block_number(conn: &Connection) -> Result<Option<u64>, rusqlite::Error> {
    conn.query_row(sql::query::GET_LATEST_BLOCK_NUMBER, [], |row| {
        row.get::<_, Option<u64>>("number")
    })
}

/// Fetches the latest block.
pub fn get_latest_block(conn: &Transaction) -> Result<Option<Block>, QueryError> {
    let Some(block_number) = get_latest_block_number(conn)? else {
        return Ok(None);
    };
    let blocks = list_blocks(conn, block_number..block_number.saturating_add(1))?;
    Ok(blocks.into_iter().next())
}

/// Fetches the last progress on state derivation.
pub fn get_state_progress(conn: &Connection) -> Result<Option<(u64, ContentAddress)>, QueryError> {
    let mut stmt = conn.prepare(sql::query::GET_STATE_PROGRESS)?;
    let value_blob: Option<(i64, Vec<u8>)> = stmt
        .query_row([], |row| Ok((row.get("number")?, row.get("block_hash")?)))
        .optional()?;
    value_blob
        .map(|(number, block_hash_blob)| {
            Ok((
                number as u64,
                decode(&block_hash_blob).map_err(QueryError::Decode)?,
            ))
        })
        .transpose()
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
/// Returns each non-empty L2 block number in the range alongside a
/// `Vec<Contract>` containing the contracts appearing in the DA proof for that
/// block.
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
            let block_num: u64 = row.get("l2_block_number")?;
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
        let (l2_block_num, ca_blob, salt_blob, pred_blob): (u64, Vec<u8>, Vec<u8>, Vec<u8>) = res?;
        let contract_ca: ContentAddress = decode(&ca_blob)?;
        let salt: Hash = decode(&salt_blob)?;

        // Fetch the block entry associated with the given block number or insert if new.
        let block = match last_block_num {
            Some(n) if n == l2_block_num => blocks.last_mut().expect("block entry must exist"),
            _ => {
                last_block_num = Some(l2_block_num);
                last_contract_ca = None;
                blocks.push((l2_block_num, vec![]));
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

#[derive(Debug)]
struct SolutionPtr {
    block_number: u64,
    solution_index: u64,
}

fn state_position_to_solution_ptr(
    conn: &Connection,
    position: &StatePosition,
) -> Result<Option<SolutionPtr>, rusqlite::Error> {
    match position {
        StatePosition::Optimistic(Optimistic { number, position }) => {
            let Some(number) = block_number_to_number(conn, number)? else {
                return Ok(None);
            };

            match (number, position) {
                (0, SolutionIndex::Start) => Ok(None),
                (number, SolutionIndex::Start) => Ok(Some(SolutionPtr {
                    block_number: number - 1,
                    solution_index: i64::MAX as u64,
                })),
                (number, SolutionIndex::End) => Ok(Some(SolutionPtr {
                    block_number: number,
                    solution_index: i64::MAX as u64,
                })),
                (number, SolutionIndex::Index(index)) => Ok(Some(SolutionPtr {
                    block_number: number,
                    solution_index: *index,
                })),
            }
        }
    }
}

fn block_number_to_number(
    conn: &Connection,
    block_number: &BlockNumber,
) -> Result<Option<u64>, rusqlite::Error> {
    match block_number {
        BlockNumber::Latest => get_latest_block_number(conn),
        BlockNumber::Number(n) => Ok(Some(*n)),
    }
}
