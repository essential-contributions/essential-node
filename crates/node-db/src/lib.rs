#![warn(missing_docs)]

//! The node's DB interface and sqlite implementation.
//!
//! The core capability of the node is to:
//!
//! 1. Receive blocks from an L1 relayer and validate them.
//! 2. Receive contracts from the p2p network so that they're available for validation.
//!
//! As a part of satisfying these requirements, this crate provides the basic
//! functions required for safely creating the necessary tables and inserting/
//! querying/updating them as necessary.

pub use error::QueryError;
use essential_hash::content_addr;
#[doc(inline)]
pub use essential_node_db_sql as sql;
use essential_types::{
    convert::{bytes_from_word, word_from_bytes},
    solution::{Mutation, Solution, SolutionData},
    Block, ContentAddress, Hash, Key, PredicateAddress, Value, Word,
};
use futures::Stream;
#[cfg(feature = "pool")]
pub use pool::ConnectionPool;
pub use query_range::address;
pub use query_range::finalized;
use rusqlite::{named_params, params, Connection, OptionalExtension, Transaction};
use std::{ops::Range, time::Duration};

mod error;
#[cfg(feature = "pool")]
pub mod pool;
mod query_range;

/// Types that may be provided to [`subscribe_blocks`] to provide access to
/// [`Connection`]s while streaming.
pub trait AcquireConnection {
    /// Asynchronously acquire a handle to a [`Connection`].
    ///
    /// Returns `Some` in the case a connection could be acquired, or `None` in
    /// the case that the connection source is no longer available.
    #[allow(async_fn_in_trait)]
    async fn acquire_connection(&self) -> Option<impl 'static + AsMut<Connection>>;
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
///
/// Returns the `ContentAddress` of the inserted block.
pub fn insert_block(tx: &Transaction, block: &Block) -> rusqlite::Result<ContentAddress> {
    // Insert the header.
    let secs = block.timestamp.as_secs();
    let nanos = block.timestamp.subsec_nanos() as u64;
    let solution_hashes: Vec<ContentAddress> = block.solutions.iter().map(content_addr).collect();
    let block_address =
        essential_hash::block_addr::from_block_and_solutions_addrs_slice(block, &solution_hashes);

    // TODO: Use real parent block address once blocks have parent hashes.
    let parent_block_address = ContentAddress([0; 32]);

    tx.execute(
        sql::insert::BLOCK,
        named_params! {
            ":block_address": block_address.0,
            ":parent_block_address": parent_block_address.0,
            ":number": block.number,
            ":timestamp_secs": secs,
            ":timestamp_nanos": nanos,
        },
    )?;

    // Insert all solutions.
    let mut stmt_solution = tx.prepare(sql::insert::SOLUTION)?;
    let mut stmt_block_solution = tx.prepare(sql::insert::BLOCK_SOLUTION)?;
    let mut stmt_solution_data = tx.prepare(sql::insert::SOLUTION_DATA)?;
    let mut stmt_mutation = tx.prepare(sql::insert::MUTATION)?;
    let mut stmt_dec_var = tx.prepare(sql::insert::DEC_VAR)?;

    for (ix, (solution, ca)) in block.solutions.iter().zip(solution_hashes).enumerate() {
        // Insert the solution.
        stmt_solution.execute(named_params! {
            ":content_hash": ca.0,
        })?;

        // Create a mapping between the block and the solution.
        stmt_block_solution.execute(named_params! {
            ":block_address": block_address.0,
            ":solution_hash": &ca.0,
            ":solution_index": ix,
        })?;

        for (data_ix, data) in solution.data.iter().enumerate() {
            stmt_solution_data.execute(named_params! {
                ":solution_hash": ca.0,
                ":data_index": data_ix,
                ":contract_addr": data.predicate_to_solve.contract.0,
                ":predicate_addr": data.predicate_to_solve.predicate.0,
            })?;
            for (mutation_ix, mutation) in data.state_mutations.iter().enumerate() {
                stmt_mutation.execute(named_params! {
                    ":solution_hash": ca.0,
                    ":data_index": data_ix,
                    ":mutation_index": mutation_ix,
                    ":key": blob_from_words(&mutation.key),
                    ":value": blob_from_words(&mutation.value),
                })?;
            }
            for (dec_var_ix, dec_var) in data.decision_variables.iter().enumerate() {
                stmt_dec_var.execute(named_params! {
                    ":solution_hash": ca.0,
                    ":data_index": data_ix,
                    ":dec_var_index": dec_var_ix,
                    ":value": blob_from_words(dec_var)
                })?;
            }
        }
    }
    stmt_solution.finalize()?;
    stmt_block_solution.finalize()?;
    stmt_solution_data.finalize()?;
    stmt_mutation.finalize()?;
    stmt_dec_var.finalize()?;

    Ok(block_address)
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

/// Updates the state for a given contract content address and key.
pub fn update_state(
    conn: &Connection,
    contract_ca: &ContentAddress,
    key: &Key,
    value: &Value,
) -> rusqlite::Result<()> {
    conn.execute(
        sql::update::STATE,
        named_params! {
            ":contract_ca": contract_ca.0,
            ":key": blob_from_words(key),
            ":value": blob_from_words(value),
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
    conn.execute(
        sql::update::DELETE_STATE,
        named_params! {
            ":contract_ca": contract_ca.0,
            ":key": blob_from_words(key),
        },
    )?;
    Ok(())
}

/// Fetches a solution by its content address.
pub fn get_solution(tx: &Transaction, ca: &ContentAddress) -> Result<Solution, QueryError> {
    let mut data_stmt = tx.prepare(sql::query::GET_SOLUTION_DATA)?;
    let mut data = data_stmt
        .query_map([ca.0], |row| {
            let contract_addr = row.get::<_, Hash>("contract_addr")?;
            let predicate_addr = row.get::<_, Hash>("predicate_addr")?;
            Ok(SolutionData {
                predicate_to_solve: PredicateAddress {
                    contract: ContentAddress(contract_addr),
                    predicate: ContentAddress(predicate_addr),
                },
                state_mutations: vec![],
                decision_variables: vec![],
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    data_stmt.finalize()?;

    let mut dec_vars_stmt = tx.prepare(sql::query::GET_SOLUTION_DEC_VARS)?;
    let mut mutations_stmt = tx.prepare(sql::query::GET_SOLUTION_MUTATIONS)?;

    for (data_ix, datum) in data.iter_mut().enumerate() {
        let mut mutations = vec![];
        let mut dec_vars = vec![];

        // Fetch the mutations.
        let mut mutation_rows = mutations_stmt.query(named_params! {
            ":content_hash": ca.0,
            ":data_index": data_ix,
        })?;
        while let Some(mutation_row) = mutation_rows.next()? {
            let key_blob: Vec<u8> = mutation_row.get("key")?;
            let value_blob: Vec<u8> = mutation_row.get("value")?;
            let key: Key = words_from_blob(&key_blob);
            let value: Value = words_from_blob(&value_blob);
            mutations.push(Mutation { key, value });
        }

        // Fetch the decision variables.
        let mut dec_var_rows = dec_vars_stmt.query(named_params! {
            ":content_hash": ca.0,
            ":data_index": data_ix,
        })?;
        while let Some(dec_var_row) = dec_var_rows.next()? {
            let value_blob: Vec<u8> = dec_var_row.get("value")?;
            let value: Value = words_from_blob(&value_blob);
            dec_vars.push(value);
        }

        datum.state_mutations = mutations;
        datum.decision_variables = dec_vars;
    }

    mutations_stmt.finalize()?;
    dec_vars_stmt.finalize()?;

    Ok(Solution { data })
}

/// Fetches the state value for the given contract content address and key pair.
pub fn query_state(
    conn: &Connection,
    contract_ca: &ContentAddress,
    key: &Key,
) -> Result<Option<Value>, QueryError> {
    use rusqlite::OptionalExtension;
    let mut stmt = conn.prepare(sql::query::GET_STATE)?;
    let value_blob: Option<Vec<u8>> = stmt
        .query_row(params![contract_ca.0, blob_from_words(key)], |row| {
            row.get("value")
        })
        .optional()?;
    Ok(value_blob.as_deref().map(words_from_blob))
}

/// Given a block address, returns the header for that block.
///
/// Returns the block number and block timestamp in that order.
pub fn get_block_header(
    conn: &Connection,
    block_address: &ContentAddress,
) -> rusqlite::Result<Option<(Word, Duration)>> {
    conn.query_row(
        sql::query::GET_BLOCK_HEADER,
        named_params! {
            ":block_address": block_address.0,
        },
        |row| {
            let number: Word = row.get("number")?;
            let timestamp_secs: u64 = row.get("timestamp_secs")?;
            let timestamp_nanos: u32 = row.get("timestamp_nanos")?;
            let timestamp = Duration::new(timestamp_secs, timestamp_nanos);
            Ok((number, timestamp))
        },
    )
    .optional()
}

/// Returns the block with given address.
pub fn get_block(
    tx: &Transaction,
    block_address: &ContentAddress,
) -> Result<Option<Block>, QueryError> {
    let mut stmt = tx.prepare(sql::query::GET_BLOCK)?;
    let rows = stmt.query_map(
        named_params! {
            ":block_address": block_address.0,
        },
        |row| {
            let block_number: Word = row.get("number")?;
            let timestamp_secs: u64 = row.get("timestamp_secs")?;
            let timestamp_nanos: u32 = row.get("timestamp_nanos")?;
            let solution_hash: Hash = row.get("content_hash")?;
            let timestamp = Duration::new(timestamp_secs, timestamp_nanos);
            Ok((block_number, timestamp, ContentAddress(solution_hash)))
        },
    )?;

    let mut block_in_progress = Option::None;
    for res in rows {
        let (block_number, timestamp, solution_addr): (Word, Duration, ContentAddress) = res?;

        if block_in_progress.is_none() {
            block_in_progress = Some(Block {
                number: block_number,
                timestamp,
                solutions: vec![],
            });
        }
        let mut block = block_in_progress.expect("should have been set above at worst case");
        // Add the solution.
        // If there are performance issues, use statements in `get_solution` directly.
        // See https://github.com/essential-contributions/essential-node/issues/154.
        let solution = get_solution(tx, &solution_addr)?;
        block.solutions.push(solution);
        block_in_progress = Some(block);
    }
    Ok(block_in_progress)
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

/// Fetches the parent block address.
pub fn get_parent_block_address(
    conn: &Connection,
    block_address: &ContentAddress,
) -> Result<Option<ContentAddress>, rusqlite::Error> {
    conn.query_row(
        sql::query::GET_PARENT_BLOCK_ADDRESS,
        named_params! {
            ":block_address": block_address.0,
        },
        |row| row.get::<_, Hash>("block_address").map(ContentAddress),
    )
    .optional()
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

/// Given a block address, returns the addresses of blocks that have the next block number.
pub fn get_next_block_addresses(
    conn: &Connection,
    current_block: &ContentAddress,
) -> Result<Vec<ContentAddress>, QueryError> {
    let mut stmt = conn.prepare(sql::query::GET_NEXT_BLOCK_ADDRESSES)?;
    let rows = stmt.query_map(
        named_params! {
            ":current_block": current_block.0,
        },
        |row| {
            let block_address: Hash = row.get("block_address")?;
            Ok(block_address)
        },
    )?;
    let block_addresses = rows
        .collect::<Result<Vec<_>, _>>()?
        .iter()
        .map(|hash| ContentAddress(*hash))
        .collect();
    Ok(block_addresses)
}

/// Lists all blocks in the given range.
pub fn list_blocks(tx: &Transaction, block_range: Range<Word>) -> Result<Vec<Block>, QueryError> {
    let mut stmt = tx.prepare(sql::query::LIST_BLOCKS)?;
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
            let solution_hash: Hash = row.get("content_hash")?;
            let timestamp = Duration::new(timestamp_secs, timestamp_nanos);
            Ok((
                block_address,
                block_number,
                timestamp,
                ContentAddress(solution_hash),
            ))
        },
    )?;

    // Query yields in order of block number and solution index.
    let mut blocks: Vec<Block> = vec![];
    let mut last_block_address = None;
    for res in rows {
        let (block_address, block_number, timestamp, solution_addr): (
            essential_types::Hash,
            Word,
            Duration,
            ContentAddress,
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
        // If there are performance issues, use statements in `get_solution` directly.
        // See https://github.com/essential-contributions/essential-node/issues/154.
        let solution = get_solution(tx, &solution_addr)?;
        block.solutions.push(solution);
    }
    Ok(blocks)
}

/// Lists blocks and their solutions within a specific time range with pagination.
pub fn list_blocks_by_time(
    tx: &Transaction,
    range: Range<Duration>,
    page_size: i64,
    page_number: i64,
) -> Result<Vec<Block>, QueryError> {
    let mut stmt = tx.prepare(sql::query::LIST_BLOCKS_BY_TIME)?;
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
            let solution_hash: Hash = row.get("content_hash")?;
            let timestamp = Duration::new(timestamp_secs, timestamp_nanos);
            Ok((
                block_address,
                block_number,
                timestamp,
                ContentAddress(solution_hash),
            ))
        },
    )?;

    // Query yields in order of block number and solution index.
    let mut blocks: Vec<Block> = vec![];
    let mut last_block_address: Option<essential_types::Hash> = None;
    for res in rows {
        let (block_address, block_number, timestamp, solution_addr): (
            essential_types::Hash,
            Word,
            Duration,
            ContentAddress,
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
        // If there are performance issues, use statements in `get_solution` directly.
        // See https://github.com/essential-contributions/essential-node/issues/154.
        let solution = get_solution(tx, &solution_addr)?;
        block.solutions.push(solution);
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
    tx: &Transaction,
    block_range: Range<Word>,
) -> Result<Vec<Block>, QueryError> {
    let mut stmt = tx.prepare(sql::query::LIST_UNCHECKED_BLOCKS)?;
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
            let solution_addr: Hash = row.get("content_hash")?;
            let timestamp = Duration::new(timestamp_secs, timestamp_nanos);
            Ok((
                block_address,
                block_number,
                timestamp,
                ContentAddress(solution_addr),
            ))
        },
    )?;

    // Query yields in order of block number and solution index.
    let mut blocks: Vec<Block> = vec![];
    let mut last_block_address = None;
    for res in rows {
        let (block_address, block_number, timestamp, solution_addr): (
            essential_types::Hash,
            Word,
            Duration,
            ContentAddress,
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
        // If there are performance issues, use statements in `get_solution` directly.
        // See https://github.com/essential-contributions/essential-node/issues/154.
        let solution = get_solution(tx, &solution_addr)?;
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
    // Helper function to list blocks by block number range.
    fn list_blocks_by_conn(
        conn: &mut Connection,
        block_range: Range<Word>,
    ) -> Result<Vec<Block>, QueryError> {
        let tx = conn.transaction()?;
        let blocks = list_blocks(&tx, block_range)?;
        drop(tx);
        Ok(blocks)
    }

    let init = (start_block, acquire_conn, await_new_block);
    futures::stream::unfold(init, move |(block_ix, acq_conn, mut new_block)| {
        let next_ix = block_ix + 1;
        async move {
            loop {
                // Acquire a connection and query for the current block.
                let mut conn = acq_conn.acquire_connection().await?;
                let res = list_blocks_by_conn(conn.as_mut(), block_ix..next_ix);
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

/// Short-hand for constructing a transaction, providing it as an argument to
/// the given function, then committing the transaction before returning.
pub fn with_tx<T, E>(
    conn: &mut rusqlite::Connection,
    f: impl FnOnce(&mut Transaction) -> Result<T, E>,
) -> Result<T, E>
where
    E: From<rusqlite::Error>,
{
    let mut tx = conn.transaction()?;
    let out = f(&mut tx)?;
    tx.commit()?;
    Ok(out)
}

/// Convert a slice of `Word`s into a blob.
pub fn blob_from_words(words: &[Word]) -> Vec<u8> {
    words.iter().copied().flat_map(bytes_from_word).collect()
}
/// Convert a blob into a vector of `Word`s.
pub fn words_from_blob(bytes: &[u8]) -> Vec<Word> {
    bytes
        .chunks_exact(core::mem::size_of::<Word>())
        .map(|bytes| word_from_bytes(bytes.try_into().expect("Can't fail due to chunks exact")))
        .collect()
}
