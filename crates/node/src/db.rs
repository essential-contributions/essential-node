//! Provides the node's [`ConnectionPool`] implementation and related items.
//!
//! This module extends [`essential_node_db`] and [`rusqlite_pool::tokio`] items
//! with node-specific wrappers, short-hands and helpers.

use core::ops::Range;
use essential_node_db::{self as db};
use essential_types::{
    contract::Contract, predicate::Predicate, solution::Solution, Block, ContentAddress, Key, Value,
};
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

impl ConnectionPool {
    /// Create the connection pool from the given configuration.
    pub(crate) fn new(conf: &Config) -> rusqlite::Result<Self> {
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
    pub async fn insert_block(&self, block: Arc<Block>) -> Result<(), AcquireThenRusqliteError> {
        self.acquire_then(move |h| with_tx(h, |tx| db::insert_block(tx, &block)))
            .await
    }

    /// Insert the given contract into the `contract` table and for each of its
    /// predicates add entries to the `predicate` and `contract_predicate` tables.
    /// The `l2_block_num` is the L2 block containing the contract's DA proof.
    pub async fn insert_contract(
        &self,
        contract: Arc<Contract>,
        l2_block_num: u64,
    ) -> Result<(), AcquireThenRusqliteError> {
        self.acquire_then(move |h| {
            with_tx(h, |tx| db::insert_contract(tx, &contract, l2_block_num))
        })
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

    /// Fetches a contract by its content address.
    pub async fn get_contract(
        &self,
        ca: ContentAddress,
    ) -> Result<Option<Contract>, AcquireThenQueryError> {
        self.acquire_then(move |h| db::get_contract(h, &ca)).await
    }

    /// Fetches a predicate by its predicate content address.
    pub async fn get_predicate(
        &self,
        predicate_ca: ContentAddress,
    ) -> Result<Option<Predicate>, AcquireThenQueryError> {
        self.acquire_then(move |h| db::get_predicate(h, &predicate_ca))
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

    /// Get the state progress, returning the last block number and hash.
    pub async fn get_state_progress(
        &self,
    ) -> Result<Option<ContentAddress>, AcquireThenQueryError> {
        self.acquire_then(|h| db::get_state_progress(h)).await
    }

    /// Lists all blocks in the given range.
    pub async fn list_blocks(
        &self,
        block_range: Range<u64>,
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

    /// Lists contracts and their predicates within a given DA block range.
    ///
    /// Returns each non-empty DA block number in the range alongside a
    /// `Vec<Contract>` containing the contracts appearing in that block.
    pub async fn list_contracts(
        &self,
        block_range: Range<u64>,
    ) -> Result<Vec<(u64, Vec<Contract>)>, AcquireThenQueryError> {
        self.acquire_then(move |h| db::list_contracts(h, block_range))
            .await
    }
}

impl Config {
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
    match source {
        Source::Memory(id) => new_mem_conn(id),
        Source::Path(p) => rusqlite::Connection::open(p),
    }
}

/// Create an in-memory connection with the given ID
fn new_mem_conn(id: &str) -> rusqlite::Result<rusqlite::Connection> {
    let conn_str = format!("file:/{id}");
    rusqlite::Connection::open_with_flags_and_vfs(conn_str, Default::default(), "memdb")
}
