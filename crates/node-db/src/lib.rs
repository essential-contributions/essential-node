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
use essential_node_types::{block, Block, BlockHeader};
use essential_types::{
    convert::{bytes_from_word, word_from_bytes},
    solution::{Mutation, Solution, SolutionSet},
    ContentAddress, Hash, Key, PredicateAddress, Value, Word,
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
/// 2. Insert each of its solution sets into the `solution_set` and `block_solution_set` tables.
///
/// Returns the `ContentAddress` of the inserted block.
pub fn insert_block(tx: &Transaction, block: &Block) -> rusqlite::Result<ContentAddress> {
    // Insert the header.
    let secs = block.header.timestamp.as_secs();
    let nanos = block.header.timestamp.subsec_nanos() as u64;
    let solution_set_addrs: Vec<ContentAddress> =
        block.solution_sets.iter().map(content_addr).collect();
    let block_address = block::addr::from_header_and_solution_set_addrs_slice(
        &block.header,
        &solution_set_addrs,
    );

    // TODO: Use real parent block address once blocks have parent addresses.
    let parent_block_address = ContentAddress([0; 32]);

    tx.execute(
        sql::insert::BLOCK,
        named_params! {
            ":block_address": block_address.0,
            ":parent_block_address": parent_block_address.0,
            ":number": block.header.number,
            ":timestamp_secs": secs,
            ":timestamp_nanos": nanos,
        },
    )?;

    // Insert all solution sets.
    let mut stmt_solution_set = tx.prepare(sql::insert::SOLUTION_SET)?;
    let mut stmt_block_solution_set = tx.prepare(sql::insert::BLOCK_SOLUTION_SET)?;
    let mut stmt_solution = tx.prepare(sql::insert::SOLUTION)?;
    let mut stmt_mutation = tx.prepare(sql::insert::MUTATION)?;
    let mut stmt_pred_data = tx.prepare(sql::insert::PRED_DATA)?;

    for (ix, (solution_set, ca)) in block
        .solution_sets
        .iter()
        .zip(solution_set_addrs)
        .enumerate()
    {
        // Insert the solution set.
        stmt_solution_set.execute(named_params! {
            ":content_addr": ca.0,
        })?;

        // Create a mapping between the block and the solution set.
        stmt_block_solution_set.execute(named_params! {
            ":block_address": block_address.0,
            ":solution_set_addr": &ca.0,
            ":solution_set_index": ix,
        })?;

        // Insert solutions.
        for (solution_ix, solution) in solution_set.solutions.iter().enumerate() {
            stmt_solution.execute(named_params! {
                ":solution_set_addr": ca.0,
                ":solution_index": solution_ix,
                ":contract_addr": solution.predicate_to_solve.contract.0,
                ":predicate_addr": solution.predicate_to_solve.predicate.0,
            })?;
            for (mutation_ix, mutation) in solution.state_mutations.iter().enumerate() {
                stmt_mutation.execute(named_params! {
                    ":solution_set_addr": ca.0,
                    ":solution_index": solution_ix,
                    ":mutation_index": mutation_ix,
                    ":key": blob_from_words(&mutation.key),
                    ":value": blob_from_words(&mutation.value),
                })?;
            }
            for (pred_data_ix, pred_data) in solution.predicate_data.iter().enumerate() {
                stmt_pred_data.execute(named_params! {
                    ":solution_set_addr": ca.0,
                    ":solution_index": solution_ix,
                    ":pred_data_index": pred_data_ix,
                    ":value": blob_from_words(pred_data)
                })?;
            }
        }
    }
    stmt_solution_set.finalize()?;
    stmt_block_solution_set.finalize()?;
    stmt_solution.finalize()?;
    stmt_mutation.finalize()?;
    stmt_pred_data.finalize()?;

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
    solution_set_addr: &ContentAddress,
) -> rusqlite::Result<()> {
    conn.execute(
        sql::insert::FAILED_BLOCK,
        named_params! {
            ":block_address": block_address.0,
            ":solution_set_addr": solution_set_addr.0,
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

/// Fetches a solution set by its content address.
pub fn get_solution_set(tx: &Transaction, ca: &ContentAddress) -> Result<SolutionSet, QueryError> {
    let mut solution_stmt = tx.prepare(sql::query::GET_SOLUTION)?;
    let mut solutions = solution_stmt
        .query_map([ca.0], |row| {
            let contract_addr = row.get::<_, Hash>("contract_addr")?;
            let predicate_addr = row.get::<_, Hash>("predicate_addr")?;
            Ok(Solution {
                predicate_to_solve: PredicateAddress {
                    contract: ContentAddress(contract_addr),
                    predicate: ContentAddress(predicate_addr),
                },
                state_mutations: vec![],
                predicate_data: vec![],
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    solution_stmt.finalize()?;

    let mut pred_data_stmt = tx.prepare(sql::query::GET_SOLUTION_PRED_DATA)?;
    let mut mutations_stmt = tx.prepare(sql::query::GET_SOLUTION_MUTATIONS)?;

    for (solution_ix, solution) in solutions.iter_mut().enumerate() {
        // Fetch the mutations.
        let mut mutation_rows = mutations_stmt.query(named_params! {
            ":content_addr": ca.0,
            ":solution_index": solution_ix,
        })?;
        while let Some(mutation_row) = mutation_rows.next()? {
            let key_blob: Vec<u8> = mutation_row.get("key")?;
            let value_blob: Vec<u8> = mutation_row.get("value")?;
            let key: Key = words_from_blob(&key_blob);
            let value: Value = words_from_blob(&value_blob);
            solution.state_mutations.push(Mutation { key, value });
        }

        // Fetch the predicate data.
        let mut pred_data_rows = pred_data_stmt.query(named_params! {
            ":content_addr": ca.0,
            ":solution_index": solution_ix,
        })?;
        while let Some(pred_data_row) = pred_data_rows.next()? {
            let value_blob: Vec<u8> = pred_data_row.get("value")?;
            let value: Value = words_from_blob(&value_blob);
            solution.predicate_data.push(value);
        }
    }

    mutations_stmt.finalize()?;
    pred_data_stmt.finalize()?;

    Ok(SolutionSet { solutions })
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
) -> rusqlite::Result<Option<BlockHeader>> {
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
            Ok(BlockHeader { number, timestamp })
        },
    )
    .optional()
}

/// Returns the block with given address.
pub fn get_block(
    tx: &Transaction,
    block_address: &ContentAddress,
) -> Result<Option<Block>, QueryError> {
    let Some(header) = get_block_header(tx, block_address)? else {
        return Ok(None);
    };
    let mut stmt = tx.prepare(sql::query::GET_BLOCK)?;
    let rows = stmt.query_map(
        named_params! {
            ":block_address": block_address.0,
        },
        |row| {
            let solution_addr: Hash = row.get("content_addr")?;
            Ok(ContentAddress(solution_addr))
        },
    )?;

    let mut block = Block {
        header,
        solution_sets: vec![],
    };
    for res in rows {
        let solution_set_addr: ContentAddress = res?;

        // Add the solution set.
        // If there are performance issues, use statements in `get_solution_set` directly.
        // See https://github.com/essential-contributions/essential-node/issues/154.
        let solution_set = get_solution_set(tx, &solution_set_addr)?;
        block.solution_sets.push(solution_set);
    }
    Ok(Some(block))
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
            let solution_set_addr: Hash = row.get("content_addr")?;
            let timestamp = Duration::new(timestamp_secs, timestamp_nanos);
            Ok((
                block_address,
                block_number,
                timestamp,
                ContentAddress(solution_set_addr),
            ))
        },
    )?;

    // Query yields in order of block number and solution set index.
    let mut blocks: Vec<Block> = vec![];
    let mut last_block_address = None;
    for res in rows {
        let (block_address, block_number, timestamp, solution_set_addr): (
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
                    header: BlockHeader {
                        number: block_number,
                        timestamp,
                    },
                    solution_sets: vec![],
                });
                blocks.last_mut().expect("last block must exist")
            }
        };

        // Add the solution set.
        // If there are performance issues, use statements in `get_solution_set` directly.
        // See https://github.com/essential-contributions/essential-node/issues/154.
        let solution_set = get_solution_set(tx, &solution_set_addr)?;
        block.solution_sets.push(solution_set);
    }
    Ok(blocks)
}

/// Lists blocks and their solution sets within a specific time range with pagination.
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
            let solution_set_addr: Hash = row.get("content_addr")?;
            let timestamp = Duration::new(timestamp_secs, timestamp_nanos);
            Ok((
                block_address,
                block_number,
                timestamp,
                ContentAddress(solution_set_addr),
            ))
        },
    )?;

    // Query yields in order of block number and solution set index.
    let mut blocks: Vec<Block> = vec![];
    let mut last_block_address: Option<essential_types::Hash> = None;
    for res in rows {
        let (block_address, block_number, timestamp, solution_set_addr): (
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
                    header: BlockHeader {
                        number: block_number,
                        timestamp,
                    },
                    solution_sets: vec![],
                });
                blocks.last_mut().expect("last block must exist")
            }
        };

        // Add the solution set.
        // If there are performance issues, use statements in `get_solution_set` directly.
        // See https://github.com/essential-contributions/essential-node/issues/154.
        let solution_set = get_solution_set(tx, &solution_set_addr)?;
        block.solution_sets.push(solution_set);
    }
    Ok(blocks)
}

/// List failed blocks as (block number, solution set hash) within a given range.
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
            let solution_set_addr: Hash = row.get("content_addr")?;
            Ok((block_number, ContentAddress(solution_set_addr)))
        },
    )?;

    let mut failed_blocks = vec![];
    for res in rows {
        let (block_number, solution_set_addr) = res?;
        failed_blocks.push((block_number, solution_set_addr));
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
            let solution_set_addr: Hash = row.get("content_addr")?;
            let timestamp = Duration::new(timestamp_secs, timestamp_nanos);
            Ok((
                block_address,
                block_number,
                timestamp,
                ContentAddress(solution_set_addr),
            ))
        },
    )?;

    // Query yields in order of block number and solution set index.
    let mut blocks: Vec<Block> = vec![];
    let mut last_block_address = None;
    for res in rows {
        let (block_address, block_number, timestamp, solution_set_addr): (
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
                    header: BlockHeader {
                        number: block_number,
                        timestamp,
                    },
                    solution_sets: vec![],
                });
                blocks.last_mut().expect("last block must exist")
            }
        };

        // Add the solution set.
        // If there are performance issues, use statements in `get_solution_set` directly.
        // See https://github.com/essential-contributions/essential-node/issues/154.
        let solution_set = get_solution_set(tx, &solution_set_addr)?;
        block.solution_sets.push(solution_set);
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

/// Short-hand for constructing a transaction, providing it as an argument to
/// the given function, then dropping the transaction before returning.
pub fn with_tx_dropped<T, E>(
    conn: &mut rusqlite::Connection,
    f: impl FnOnce(&mut Transaction) -> Result<T, E>,
) -> Result<T, E>
where
    E: From<rusqlite::Error>,
{
    let mut tx = conn.transaction()?;
    let out = f(&mut tx)?;
    drop(tx);
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
