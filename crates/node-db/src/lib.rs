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
    Value, Word,
};
use futures::Stream;
use rusqlite::{named_params, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use std::{ops::Range, time::Duration};

pub use query_range::finalized;

mod error;
mod query_range;

/// Types that may be provided to [`subscribe_blocks`] to provide access to
/// [`Connection`]s while streaming.
pub trait AcquireConnection {
    /// Asynchronously acquire a handle to a [`Connection`].
    ///
    /// Returns `Some` in the case a connection could be acquired, or `None` in
    /// the case that the connection source is no longer available.
    #[allow(async_fn_in_trait)]
    async fn acquire_connection(&self) -> Option<impl 'static + AsRef<Connection>>;
}

/// Types that may be provided to [`subscribe_blocks`] to asynchronously await
/// the availability of a new block.
pub trait AwaitNewBlock {
    /// Wait for a new block to become available.
    ///
    /// Returns a future that resolves to `Some` when a new block is ready, or
    /// `None` when the notification source is no longer available.
    #[allow(async_fn_in_trait)]
    async fn await_new_block(&mut self) -> Option<()>;
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
    let solution_hashes: Vec<ContentAddress> = block.solutions.iter().map(content_addr).collect();
    let block_address =
        essential_hash::block_addr::from_block_and_solutions_addrs_slice(block, &solution_hashes);
    tx.execute(
        sql::insert::BLOCK,
        named_params! {
            ":block_address": block_address.0,
            ":number": block.number,
            ":timestamp_secs": secs,
            ":timestamp_nanos": nanos,
        },
    )?;

    // Insert all solutions.
    let mut stmt_solution = tx.prepare(sql::insert::SOLUTION)?;
    let mut stmt_block_solution = tx.prepare(sql::insert::BLOCK_SOLUTION)?;
    let mut stmt_mutation = tx.prepare(sql::insert::MUTATION)?;
    let mut stmt_dec_var = tx.prepare(sql::insert::DEC_VAR)?;
    let mut stmt_pub_var = tx.prepare(sql::insert::PUB_VAR)?;

    for (ix, (solution, ca)) in block.solutions.iter().zip(solution_hashes).enumerate() {
        // Insert the solution.
        let solution_blob = encode(solution);
        stmt_solution.execute(named_params! {
            ":content_hash": ca.0,
            ":solution": solution_blob,
        })?;

        // Create a mapping between the block and the solution.
        stmt_block_solution.execute(named_params! {
            ":block_address": block_address.0,
            ":solution_hash": &ca.0,
            ":solution_index": ix,
        })?;

        for (data_ix, data) in solution.data.iter().enumerate() {
            let contract_ca_blob = encode(&data.predicate_to_solve.contract);
            for (mutation_ix, mutation) in data.state_mutations.iter().enumerate() {
                let key_blob = encode(&mutation.key);
                let value_blob = encode(&mutation.value);
                stmt_mutation.execute(named_params! {
                    ":solution_hash": ca.0,
                    ":data_index": data_ix,
                    ":mutation_index": mutation_ix,
                    ":contract_ca": contract_ca_blob,
                    ":key": key_blob,
                    ":value": value_blob,
                })?;
            }
            for (dec_var_ix, dec_var) in data.decision_variables.iter().enumerate() {
                let blob = encode(&dec_var);
                stmt_dec_var.execute(named_params! {
                    ":solution_hash": ca.0,
                    ":data_index": data_ix,
                    ":dec_var_index": dec_var_ix,
                    ":value": blob
                })?;
            }
            for pub_var in &data.transient_data {
                let key_blob = encode(&pub_var.key);
                let value_blob = encode(&pub_var.value);
                stmt_pub_var.execute(named_params! {
                    ":solution_hash": ca.0,
                    ":data_index": data_ix,
                    ":key": key_blob,
                    ":value": value_blob,
                })?;
            }
        }
    }
    stmt_solution.finalize()?;
    stmt_block_solution.finalize()?;
    stmt_mutation.finalize()?;
    stmt_dec_var.finalize()?;
    stmt_pub_var.finalize()?;

    Ok(())
}

/// Finalizes the block with the given hash.
/// This sets the block to be the only block at a particular block number.
pub fn finalize_block(conn: &Connection, block_address: &ContentAddress) -> rusqlite::Result<()> {
    conn.execute(
        sql::insert::FINALIZE_BLOCK,
        named_params! {
            ":block_address": block_address.0,
        },
    )?;
    Ok(())
}

/// Inserts a failed block.
pub fn insert_failed_block(
    conn: &Connection,
    block_address: &ContentAddress,
    solution_hash: &ContentAddress,
) -> rusqlite::Result<()> {
    conn.execute(
        sql::insert::FAILED_BLOCK,
        named_params! {
            ":block_address": block_address.0,
            ":solution_hash": solution_hash.0,
        },
    )?;
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
    l2_block_number: Word,
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
    l2_block_number: Word,
    contract_ca: &ContentAddress,
) -> rusqlite::Result<()> {
    let contract_ca_blob = encode(contract_ca);

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
    block_address: &ContentAddress,
) -> rusqlite::Result<()> {
    conn.execute(
        sql::insert::STATE_PROGRESS,
        named_params! {
            ":block_address": block_address.0,
        },
    )?;
    Ok(())
}

/// Updates the progress on validation.
pub fn update_validation_progress(
    conn: &Connection,
    block_address: &ContentAddress,
) -> rusqlite::Result<()> {
    conn.execute(
        sql::insert::VALIDATION_PROGRESS,
        named_params! {
            ":block_address": block_address.0,
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
) -> Result<Option<(Word, ContentAddress)>, QueryError> {
    let Some((l2_block_number, content_hash)) = conn
        .query_row(sql::query::GET_CONTRACT_PROGRESS, [], |row| {
            let l2_block_number: Word = row.get("l2_block_number")?;
            let content_hash: Vec<u8> = row.get("content_hash")?;
            Ok((l2_block_number, content_hash))
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
    let mut stmt = conn.prepare(sql::query::GET_SOLUTION)?;
    let solution_blob: Option<Vec<u8>> = stmt
        .query_row([ca.0], |row| row.get("solution"))
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

/// Fetches the block number for a given block.
pub fn get_block_number(
    conn: &Connection,
    block_address: &ContentAddress,
) -> Result<Option<Word>, rusqlite::Error> {
    conn.query_row(
        sql::query::GET_BLOCK_NUMBER,
        named_params! {
            ":block_address": block_address.0,
        },
        |row| row.get::<_, Option<Word>>("number"),
    )
}

/// Fetches the latest finalized block hash.
pub fn get_latest_finalized_block_address(
    conn: &Connection,
) -> Result<Option<ContentAddress>, rusqlite::Error> {
    conn.query_row(sql::query::GET_LATEST_FINALIZED_BLOCK_ADDRESS, [], |row| {
        row.get::<_, Hash>("block_address").map(ContentAddress)
    })
    .optional()
}

/// Fetches the last progress on state derivation.
pub fn get_state_progress(conn: &Connection) -> Result<Option<ContentAddress>, QueryError> {
    let mut stmt = conn.prepare(sql::query::GET_STATE_PROGRESS)?;
    let value: Option<ContentAddress> = stmt
        .query_row([], |row| {
            let block_address: Hash = row.get("block_address")?;
            Ok(ContentAddress(block_address))
        })
        .optional()?;
    Ok(value)
}

/// Fetches the last progress on validation.
pub fn get_validation_progress(conn: &Connection) -> Result<Option<ContentAddress>, QueryError> {
    let mut stmt = conn.prepare(sql::query::GET_VALIDATION_PROGRESS)?;
    let value: Option<ContentAddress> = stmt
        .query_row([], |row| {
            let block_address: Hash = row.get("block_address")?;
            Ok(ContentAddress(block_address))
        })
        .optional()?;
    Ok(value)
}

/// Lists all blocks in the given range.
pub fn list_blocks(conn: &Connection, block_range: Range<Word>) -> Result<Vec<Block>, QueryError> {
    let mut stmt = conn.prepare(sql::query::LIST_BLOCKS)?;
    let rows = stmt.query_map(
        named_params! {
            ":start_block": block_range.start,
            ":end_block": block_range.end,
        },
        |row| {
            let block_address: essential_types::Hash = row.get("block_address")?;
            let block_number: Word = row.get("number")?;
            let timestamp_secs: u64 = row.get("timestamp_secs")?;
            let timestamp_nanos: u32 = row.get("timestamp_nanos")?;
            let solution_blob: Vec<u8> = row.get("solution")?;
            let timestamp = Duration::new(timestamp_secs, timestamp_nanos);
            Ok((block_address, block_number, timestamp, solution_blob))
        },
    )?;

    // Query yields in order of block number and solution index.
    let mut blocks: Vec<Block> = vec![];
    let mut last_block_address = None;
    for res in rows {
        let (block_address, block_number, timestamp, solution_blob): (
            essential_types::Hash,
            Word,
            Duration,
            Vec<u8>,
        ) = res?;

        // Fetch the block associated with the block number, inserting it first if new.
        let block = match last_block_address {
            Some(b) if b == block_address => blocks.last_mut().expect("last block must exist"),
            _ => {
                last_block_address = Some(block_address);
                blocks.push(Block {
                    number: block_number,
                    timestamp,
                    solutions: vec![],
                });
                blocks.last_mut().expect("last block must exist")
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
            let block_address: essential_types::Hash = row.get("block_address")?;
            let block_number: Word = row.get("number")?;
            let timestamp_secs: u64 = row.get("timestamp_secs")?;
            let timestamp_nanos: u32 = row.get("timestamp_nanos")?;
            let solution_blob: Vec<u8> = row.get("solution")?;
            let timestamp = Duration::new(timestamp_secs, timestamp_nanos);
            Ok((block_address, block_number, timestamp, solution_blob))
        },
    )?;

    // Query yields in order of block number and solution index.
    let mut blocks: Vec<Block> = vec![];
    let mut last_block_address: Option<essential_types::Hash> = None;
    for res in rows {
        let (block_address, block_number, timestamp, solution_blob): (
            essential_types::Hash,
            Word,
            Duration,
            Vec<u8>,
        ) = res?;

        // Fetch the block associated with the block number, inserting it first if new.
        let block = match last_block_address {
            Some(n) if n == block_address => blocks.last_mut().expect("last block must exist"),
            _ => {
                last_block_address = Some(block_address);
                blocks.push(Block {
                    number: block_number,
                    timestamp,
                    solutions: vec![],
                });
                blocks.last_mut().expect("last block must exist")
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
    block_range: Range<Word>,
) -> Result<Vec<(Word, Vec<Contract>)>, QueryError> {
    let mut stmt = conn.prepare(sql::query::LIST_CONTRACTS)?;
    let rows = stmt.query_map(
        named_params! {
            ":start_block": block_range.start,
            ":end_block": block_range.end,
        },
        |row| {
            let block_num: Word = row.get("l2_block_number")?;
            let salt_blob: Vec<u8> = row.get("salt")?;
            let contract_ca_blob: Vec<u8> = row.get("content_hash")?;
            let pred_blob: Vec<u8> = row.get("predicate")?;
            Ok((block_num, contract_ca_blob, salt_blob, pred_blob))
        },
    )?;

    // Query yields in order of block number and predicate ID.
    let mut blocks: Vec<(Word, Vec<Contract>)> = vec![];
    let mut last_block_num: Option<Word> = None;
    let mut last_contract_ca = None;
    for res in rows {
        let (l2_block_num, ca_blob, salt_blob, pred_blob): (Word, Vec<u8>, Vec<u8>, Vec<u8>) = res?;
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

/// List failed blocks as (block number, solution hash) within a given range.
pub fn list_failed_blocks(
    conn: &Connection,
    block_range: Range<Word>,
) -> Result<Vec<(Word, ContentAddress)>, QueryError> {
    let mut stmt = conn.prepare(sql::query::LIST_FAILED_BLOCKS)?;
    let rows = stmt.query_map(
        named_params! {
            ":start_block": block_range.start,
            ":end_block": block_range.end,
        },
        |row| {
            let block_number: Word = row.get("number")?;
            let solution_hash: Hash = row.get("content_hash")?;
            Ok((block_number, ContentAddress(solution_hash)))
        },
    )?;

    let mut failed_blocks = vec![];
    for res in rows {
        let (block_number, solution_hash) = res?;
        failed_blocks.push((block_number, solution_hash));
    }
    Ok(failed_blocks)
}

/// Lists all unchecked blocks in the given range.
pub fn list_unchecked_blocks(
    conn: &Connection,
    block_range: Range<Word>,
) -> Result<Vec<Block>, QueryError> {
    let mut stmt = conn.prepare(sql::query::LIST_UNCHECKED_BLOCKS)?;
    let rows = stmt.query_map(
        named_params! {
            ":start_block": block_range.start,
            ":end_block": block_range.end,
        },
        |row| {
            let block_address: essential_types::Hash = row.get("block_address")?;
            let block_number: Word = row.get("number")?;
            let timestamp_secs: u64 = row.get("timestamp_secs")?;
            let timestamp_nanos: u32 = row.get("timestamp_nanos")?;
            let solution_blob: Vec<u8> = row.get("solution")?;
            let timestamp = Duration::new(timestamp_secs, timestamp_nanos);
            Ok((block_address, block_number, timestamp, solution_blob))
        },
    )?;

    // Query yields in order of block number and solution index.
    let mut blocks: Vec<Block> = vec![];
    let mut last_block_address = None;
    for res in rows {
        let (block_address, block_number, timestamp, solution_blob): (
            essential_types::Hash,
            Word,
            Duration,
            Vec<u8>,
        ) = res?;

        // Fetch the block associated with the block number, inserting it first if new.
        let block = match last_block_address {
            Some(b) if b == block_address => blocks.last_mut().expect("last block must exist"),
            _ => {
                last_block_address = Some(block_address);
                blocks.push(Block {
                    number: block_number,
                    timestamp,
                    solutions: vec![],
                });
                blocks.last_mut().expect("last block must exist")
            }
        };

        // Add the solution.
        let solution: Solution = decode(&solution_blob)?;
        block.solutions.push(solution);
    }
    Ok(blocks)
}

/// Subscribe to all blocks from the given starting block number.
///
/// The given `acquire_conn` type will be used on each iteration to
/// asynchronously acquire a handle to a new rusqlite `Connection` from a source
/// such as a connection pool. If the returned future completes with `None`, it
/// is assumed the source of `Connection`s has closed, and in turn the `Stream`
/// will close.
///
/// The given `await_new_block` type will be used as a signal to check whether
/// or not a new block is available within the DB. The returned stream will
/// yield immediately for each block until a DB query indicates there are no
/// more blocks, at which point `await_new_block` is called before continuing.
/// If `await_new_block` returns `None`, the source of new block notifications
/// is assumed to have been closed and the stream will close.
pub fn subscribe_blocks(
    start_block: Word,
    acquire_conn: impl AcquireConnection,
    await_new_block: impl AwaitNewBlock,
) -> impl Stream<Item = Result<Block, QueryError>> {
    let init = (start_block, acquire_conn, await_new_block);
    futures::stream::unfold(init, move |(block_ix, acq_conn, mut new_block)| {
        let next_ix = block_ix + 1;
        async move {
            loop {
                // Acquire a connection and query for the current block.
                let conn = acq_conn.acquire_connection().await?;
                let res = list_blocks(conn.as_ref(), block_ix..next_ix);
                // Drop the connection ASAP in case it needs returning to a pool.
                std::mem::drop(conn);
                match res {
                    // If some error occurred, emit the error.
                    Err(err) => return Some((Err(err), (block_ix, acq_conn, new_block))),
                    // If the query succeeded, pop the single block.
                    Ok(mut vec) => match vec.pop() {
                        // If there were no matching blocks, await the next.
                        None => new_block.await_new_block().await?,
                        // If we have the block, emit it.
                        Some(block) => return Some((Ok(block), (next_ix, acq_conn, new_block))),
                    },
                }
            }
        }
    })
}
