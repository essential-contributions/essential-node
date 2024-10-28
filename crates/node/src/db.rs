//! Provides the node's [`ConnectionPool`] implementation and related items.
//!
//! This module extends [`essential_node_db`] and [`rusqlite_pool::tokio`] items
//! with node-specific wrappers, short-hands and helpers.

use core::ops::Range;
use essential_node_db as db;
pub use essential_node_db::{AwaitNewBlock, QueryError};
use essential_types::{solution::Solution, Block, ContentAddress, Key, Value, Word};
use futures::Stream;
use rusqlite::Transaction;
use rusqlite_pool::tokio::{AsyncConnectionHandle, AsyncConnectionPool};
use std::{path::PathBuf, sync::Arc, time::Duration};
use thiserror::Error;
use tokio::sync::{AcquireError, TryAcquireError};

/// Access to the node's DB connection pool and DB-access-related methods.
///
/// The handle is safe to clone and share between threads.
#[derive(Clone)]
pub struct ConnectionPool(pub(crate) AsyncConnectionPool);

/// A temporary connection handle to a [`Node`][`crate::Node`]'s [`ConnectionPool`].
///
/// Provides `Deref`, `DerefMut` impls for the inner [`rusqlite::Connection`].
pub struct ConnectionHandle(AsyncConnectionHandle);

/// Node configuration related to the database.
#[derive(Clone, Debug)]
pub struct Config {
    /// The number of simultaneous connections to the database to maintain.
    pub conn_limit: usize,
    /// How to source the node's database.
    pub source: Source,
}

/// The source of the node's database.
#[derive(Clone, Debug)]
pub enum Source {
    /// Use an in-memory database using the given string as a unique ID.
    Memory(String),
    /// Use the database at the given path.
    Path(PathBuf),
}

/// Any error that might occur during node DB connection pool access.
#[derive(Debug, Error)]
pub enum AcquireThenError<E> {
    /// Failed to acquire a DB connection.
    #[error("failed to acquire a DB connection: {0}")]
    Acquire(#[from] tokio::sync::AcquireError),
    /// The tokio spawn blocking task failed to join.
    #[error("failed to join task: {0}")]
    Join(#[from] tokio::task::JoinError),
    /// The error returned by the `acquire_then` function result.
    #[error("{0}")]
    Inner(E),
}

/// An `acquire_then` error whose function returns a result with a rusqlite error.
pub type AcquireThenRusqliteError = AcquireThenError<rusqlite::Error>;

/// An `acquire_then` error whose function returns a result with a query error.
pub type AcquireThenQueryError = AcquireThenError<db::QueryError>;

/// One or more connections failed to close.
#[derive(Debug, Error)]
pub struct ConnectionCloseErrors(pub Vec<(rusqlite::Connection, rusqlite::Error)>);

impl ConnectionPool {
    /// Create the connection pool from the given configuration.
    pub fn new(conf: &Config) -> rusqlite::Result<Self> {
        Ok(Self(new_conn_pool(conf)?))
    }

    /// Acquire a temporary database [`ConnectionHandle`] from the inner pool.
    ///
    /// In the case that all connections are busy, waits for the first available
    /// connection.
    pub async fn acquire(&self) -> Result<ConnectionHandle, AcquireError> {
        self.0.acquire().await.map(ConnectionHandle)
    }

    /// Attempt to synchronously acquire a temporary database [`ConnectionHandle`]
    /// from the inner pool.
    ///
    /// Returns `Err` in the case that all database connections are busy or if
    /// the node has been closed.
    pub fn try_acquire(&self) -> Result<ConnectionHandle, TryAcquireError> {
        self.0.try_acquire().map(ConnectionHandle)
    }

    /// Close a connection pool, returning a `ConnectionCloseErrors` in the case of any errors.
    pub fn close(&self) -> Result<(), ConnectionCloseErrors> {
        let res = self.0.close();
        let errs: Vec<_> = res.into_iter().filter_map(Result::err).collect();
        if !errs.is_empty() {
            return Err(ConnectionCloseErrors(errs));
        }
        Ok(())
    }
}

/// Short-hand methods for async DB access.
impl ConnectionPool {
    /// Asynchronous access to the node's DB via the given function.
    ///
    /// Requests and awaits a connection from the connection pool, then spawns a
    /// blocking task for the given function providing access to the connection handle.
    pub async fn acquire_then<F, T, E>(&self, f: F) -> Result<T, AcquireThenError<E>>
    where
        F: 'static + Send + FnOnce(&mut ConnectionHandle) -> Result<T, E>,
        T: 'static + Send,
        E: 'static + Send,
    {
        // Acquire a handle.
        let mut handle = self.acquire().await?;

        // Spawn the given DB connection access function on a task.
        tokio::task::spawn_blocking(move || f(&mut handle))
            .await?
            .map_err(AcquireThenError::Inner)
    }

    /// Create all database tables.
    pub async fn create_tables(&self) -> Result<(), AcquireThenRusqliteError> {
        self.acquire_then(|h| with_tx(h, |tx| db::create_tables(tx)))
            .await
    }

    /// Insert the given block into the `block` table and for each of its
    /// solutions, add a row into the `solution` and `block_solution` tables.
    pub async fn insert_block(
        &self,
        block: Arc<Block>,
    ) -> Result<ContentAddress, AcquireThenRusqliteError> {
        self.acquire_then(move |h| with_tx(h, |tx| db::insert_block(tx, &block)))
            .await
    }

    /// Finalizes the block with the given hash.
    /// This sets the block to be the only block at a particular block number.
    pub async fn finalize_block(
        &self,
        block_ca: ContentAddress,
    ) -> Result<(), AcquireThenRusqliteError> {
        self.acquire_then(move |h| db::finalize_block(h, &block_ca))
            .await
    }

    /// Updates the state for a given contract content address and key.
    pub async fn update_state(
        &self,
        contract_ca: ContentAddress,
        key: Key,
        value: Value,
    ) -> Result<(), AcquireThenRusqliteError> {
        self.acquire_then(move |h| db::update_state(h, &contract_ca, &key, &value))
            .await
    }

    /// Deletes the state for a given contract content address and key.
    pub async fn delete_state(
        &self,
        contract_ca: ContentAddress,
        key: Key,
    ) -> Result<(), AcquireThenRusqliteError> {
        self.acquire_then(move |h| db::delete_state(h, &contract_ca, &key))
            .await
    }

    /// Fetches a block by its hash.
    pub async fn get_block(
        &self,
        block_address: ContentAddress,
    ) -> Result<Option<Block>, AcquireThenQueryError> {
        self.acquire_then(move |h| db::get_block(h, &block_address))
            .await
    }

    /// Fetches a solution by its content address.
    pub async fn get_solution(
        &self,
        ca: ContentAddress,
    ) -> Result<Option<Solution>, AcquireThenQueryError> {
        self.acquire_then(move |h| db::get_solution(h, &ca)).await
    }

    /// Fetches the state value for the given contract content address and key pair.
    pub async fn query_state(
        &self,
        contract_ca: ContentAddress,
        key: Key,
    ) -> Result<Option<Value>, AcquireThenQueryError> {
        self.acquire_then(move |h| db::query_state(h, &contract_ca, &key))
            .await
    }

    /// Fetches the state value for the given contract content address and key pair
    /// within a range of blocks.
    pub async fn query_latest_finalized_block(
        &self,
        contract_ca: ContentAddress,
        key: Key,
    ) -> Result<Option<Value>, AcquireThenQueryError> {
        self.acquire_then(move |h| {
            let tx = h.transaction()?;
            let Some(addr) = db::get_latest_finalized_block_address(&tx)? else {
                return Ok(None);
            };
            let Some((number, _)) = db::get_block_header(&tx, &addr)? else {
                return Ok(None);
            };
            let value =
                db::finalized::query_state_inclusive_block(&tx, &contract_ca, &key, number)?;
            tx.finish()?;
            Ok(value)
        })
        .await
    }

    /// Fetches the state value for the given contract content address and key pair
    /// within a range of blocks inclusive. `..=block`.
    pub async fn query_state_finalized_inclusive_block(
        &self,
        contract_ca: ContentAddress,
        key: Key,
        block_number: Word,
    ) -> Result<Option<Value>, AcquireThenQueryError> {
        self.acquire_then(move |h| {
            db::finalized::query_state_inclusive_block(h, &contract_ca, &key, block_number)
        })
        .await
    }

    /// Fetches the state value for the given contract content address and key pair
    /// within a range of blocks exclusive. `..block`.
    pub async fn query_state_finalized_exclusive_block(
        &self,
        contract_ca: ContentAddress,
        key: Key,
        block_number: Word,
    ) -> Result<Option<Value>, AcquireThenQueryError> {
        self.acquire_then(move |h| {
            db::finalized::query_state_exclusive_block(h, &contract_ca, &key, block_number)
        })
        .await
    }

    /// Fetches the state value for the given contract content address and key pair
    /// within a range of blocks and solutions inclusive. `..block[..=solution]`.
    pub async fn query_state_finalized_inclusive_solution(
        &self,
        contract_ca: ContentAddress,
        key: Key,
        block_number: Word,
        solution_ix: u64,
    ) -> Result<Option<Value>, AcquireThenQueryError> {
        self.acquire_then(move |h| {
            db::finalized::query_state_inclusive_solution(
                h,
                &contract_ca,
                &key,
                block_number,
                solution_ix,
            )
        })
        .await
    }

    /// Fetches the state value for the given contract content address and key pair
    /// within a range of blocks and solutions exclusive. `..=block[..solution]`.
    pub async fn query_state_finalized_exclusive_solution(
        &self,
        contract_ca: ContentAddress,
        key: Key,
        block_number: Word,
        solution_ix: u64,
    ) -> Result<Option<Value>, AcquireThenQueryError> {
        self.acquire_then(move |h| {
            db::finalized::query_state_exclusive_solution(
                h,
                &contract_ca,
                &key,
                block_number,
                solution_ix,
            )
        })
        .await
    }

    /// Get the validation progress, returning the last block hash.
    pub async fn get_validation_progress(
        &self,
    ) -> Result<Option<ContentAddress>, AcquireThenQueryError> {
        self.acquire_then(|h| db::get_validation_progress(h)).await
    }

    /// Get address of block with number 0.
    pub async fn get_big_bang_block_address(
        &self,
    ) -> Result<Option<ContentAddress>, AcquireThenQueryError> {
        self.acquire_then(|h| db::get_big_bang_block_address(h))
            .await
    }

    /// Get next block(s) given the current block hash.
    pub async fn get_next_block_address(
        &self,
        current_block: ContentAddress,
    ) -> Result<Vec<ContentAddress>, AcquireThenQueryError> {
        self.acquire_then(move |h| db::get_next_block_address(h, &current_block))
            .await
    }

    /// Update the validation progress to point to the block with the given CA.
    pub async fn update_validation_progress(
        &self,
        block_ca: ContentAddress,
    ) -> Result<(), AcquireThenRusqliteError> {
        self.acquire_then(move |h| db::update_validation_progress(h, &block_ca))
            .await
    }

    /// Lists all blocks in the given range.
    pub async fn list_blocks(
        &self,
        block_range: Range<Word>,
    ) -> Result<Vec<Block>, AcquireThenQueryError> {
        self.acquire_then(move |h| db::list_blocks(h, block_range))
            .await
    }

    /// Lists blocks and their solutions within a specific time range with pagination.
    pub async fn list_blocks_by_time(
        &self,
        range: Range<Duration>,
        page_size: i64,
        page_number: i64,
    ) -> Result<Vec<Block>, AcquireThenQueryError> {
        self.acquire_then(move |h| db::list_blocks_by_time(h, range, page_size, page_number))
            .await
    }

    /// Subscribe to all blocks from the given starting block number.
    pub fn subscribe_blocks(
        &self,
        start_block: Word,
        await_new_block: impl AwaitNewBlock,
    ) -> impl Stream<Item = Result<Block, QueryError>> {
        db::subscribe_blocks(start_block, self.clone(), await_new_block)
    }
}

impl Config {
    /// Config with specified source and connection limit.
    pub fn new(source: Source, conn_limit: usize) -> Self {
        Self { source, conn_limit }
    }

    /// The default connection limit.
    ///
    /// This default uses the number of available CPUs as a heuristic for a
    /// default connection limit. Specifically, it multiplies the number of
    /// available CPUs by 4.
    pub fn default_conn_limit() -> usize {
        // TODO: Unsure if wasm-compatible? May want a feature for this?
        num_cpus::get().saturating_mul(4)
    }
}

impl Source {
    /// A temporary, in-memory DB with a default ID.
    pub fn default_memory() -> Self {
        // Default ID cannot be an empty string.
        Self::Memory("__default-id".to_string())
    }
}

impl AsRef<rusqlite::Connection> for ConnectionHandle {
    fn as_ref(&self) -> &rusqlite::Connection {
        self
    }
}

impl core::ops::Deref for ConnectionHandle {
    type Target = AsyncConnectionHandle;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl core::ops::DerefMut for ConnectionHandle {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl essential_node_db::AcquireConnection for ConnectionPool {
    async fn acquire_connection(&self) -> Option<impl 'static + AsRef<rusqlite::Connection>> {
        self.acquire().await.ok()
    }
}

impl Default for Source {
    fn default() -> Self {
        Self::default_memory()
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            conn_limit: Self::default_conn_limit(),
            source: Source::default(),
        }
    }
}

impl core::fmt::Display for ConnectionCloseErrors {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        writeln!(f, "failed to close one or more connections:")?;
        for (ix, (_conn, err)) in self.0.iter().enumerate() {
            writeln!(f, "  {ix}: {err}")?;
        }
        Ok(())
    }
}

/// Short-hand for constructing a transaction, providing it as an argument to
/// the given function, then committing the transaction before returning.
pub(crate) fn with_tx<T, E>(
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

/// Initialise the connection pool from the given configuration.
fn new_conn_pool(conf: &Config) -> rusqlite::Result<AsyncConnectionPool> {
    AsyncConnectionPool::new(conf.conn_limit, || new_conn(&conf.source))
}

/// Create a new connection given a DB source.
pub(crate) fn new_conn(source: &Source) -> rusqlite::Result<rusqlite::Connection> {
    let conn = match source {
        Source::Memory(id) => new_mem_conn(id),
        Source::Path(p) => {
            if let Some(dir) = p.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let conn = rusqlite::Connection::open(p)?;
            conn.pragma_update(None, "trusted_schema", false)?;
            conn.pragma_update(None, "synchronous", 1)?;
            Ok(conn)
        }
    }?;
    conn.pragma_update(None, "foreign_keys", true)?;
    Ok(conn)
}

/// Create an in-memory connection with the given ID
fn new_mem_conn(id: &str) -> rusqlite::Result<rusqlite::Connection> {
    let conn_str = format!("file:/{id}");
    rusqlite::Connection::open_with_flags_and_vfs(conn_str, Default::default(), "memdb")
}
