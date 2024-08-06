//! Extends [`essential_node_db`] and [`rusqlite_pool::tokio`] items with
//! node-specific short-hands and helpers.

use core::ops::Range;
use essential_node_db as db;
use essential_types::{
    contract::Contract, predicate::Predicate, solution::Solution, Block, ContentAddress, Key, Value,
};
use rusqlite::Transaction;
use rusqlite_pool::tokio::{AsyncConnectionHandle, AsyncConnectionPool};
use std::{path::PathBuf, sync::Arc, time::Duration};
use tokio::sync::{AcquireError, TryAcquireError};

/// Access to the node's DB connection pool.
///
/// The handle is safe to clone and share between threads.
#[derive(Clone)]
pub struct ConnectionPool(pub(crate) Arc<AsyncConnectionPool>);

/// A temporary connection handle to a [`Node`]'s connection pool.
///
/// This is a thin wrapper around [`AsyncConnectionHandle`] providing short-hand
/// methods for common node DB access patterns.
///
/// A `NodeConnection` can be acquired via the [`ConnectionPool::conn`] method. It must
/// be `drop`ped in order for the inner connection to be made available to the
/// pool once more.
pub struct ConnectionHandle(AsyncConnectionHandle);

/// Node configuration related to the database.
#[derive(Clone, Debug)]
pub struct Config {
    /// The number of simultaneous connections to the database to maintain.
    pub conn_limit: usize,
    /// The source of the
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

impl ConnectionPool {
    /// Acquire a temporary database [`NodeConnection`] from the inner pool.
    ///
    /// In the case that all connections are busy, waits for the first available
    /// connection.
    pub async fn acquire(&self) -> Result<ConnectionHandle, AcquireError> {
        self.0.acquire().await.map(ConnectionHandle)
    }

    /// Attempt to synchronously acquire a temporary database [`NodeConnection`]
    /// from the inner pool.
    ///
    /// Returns `None` in the case that all database connections are busy.
    pub fn try_acquire(&self) -> Result<ConnectionHandle, TryAcquireError> {
        self.0.try_acquire().map(ConnectionHandle)
    }
}

impl ConnectionHandle {
    // NOTE: Should we expose the table creation and insertion methods?

    /// Create all database tables.
    pub fn create_tables(&mut self) -> rusqlite::Result<()> {
        self.with_tx(|tx| db::create_tables(tx))
    }

    /// Insert the given block into the `block` table and for each of its
    /// solutions, add a row into the `solution` and `block_solution` tables.
    pub fn insert_block(&mut self, block: &Block) -> rusqlite::Result<()> {
        self.with_tx(|tx| db::insert_block(tx, block))
    }

    /// Insert the given contract into the `contract` table and for each of its
    /// predicates add entries to the `predicate` and `contract_predicate` tables.
    pub fn insert_contract(
        &mut self,
        contract: &Contract,
        da_block_num: u64,
    ) -> rusqlite::Result<()> {
        self.with_tx(|tx| db::insert_contract(tx, contract, da_block_num))
    }

    /// Updates the state for a given contract content address and key.
    pub fn update_state(
        &self,
        contract_ca: &ContentAddress,
        key: &Key,
        value: &Value,
    ) -> rusqlite::Result<()> {
        db::update_state(self, contract_ca, key, value)
    }

    /// Deletes the state for a given contract content address and key.
    pub fn delete_state(&self, contract_ca: &ContentAddress, key: &Key) -> rusqlite::Result<()> {
        db::delete_state(self, contract_ca, key)
    }

    /// Fetches a contract by its content address.
    pub fn get_contract(&self, ca: &ContentAddress) -> Result<Option<Contract>, db::QueryError> {
        db::get_contract(self, ca)
    }

    /// Fetches a predicate by its predicate content address.
    pub fn get_predicate(
        &self,
        predicate_ca: &ContentAddress,
    ) -> Result<Option<Predicate>, db::QueryError> {
        db::get_predicate(self, predicate_ca)
    }

    /// Fetches a solution by its content address.
    pub fn get_solution(&self, ca: &ContentAddress) -> Result<Option<Solution>, db::QueryError> {
        db::get_solution(self, ca)
    }

    /// Fetches the state value for the given contract content address and key pair.
    pub fn query_state(
        &self,
        contract_ca: &ContentAddress,
        key: &Key,
    ) -> Result<Option<Value>, db::QueryError> {
        db::get_state_value(self, contract_ca, key)
    }

    /// Lists all blocks in the given range.
    pub fn list_blocks(&self, block_range: Range<u64>) -> Result<Vec<Block>, db::QueryError> {
        db::list_blocks(self, block_range)
    }

    /// Lists blocks and their solutions within a specific time range with pagination.
    pub fn list_blocks_by_time(
        &self,
        range: Range<Duration>,
        page_size: i64,
        page_number: i64,
    ) -> Result<Vec<Block>, db::QueryError> {
        db::list_blocks_by_time(self, range, page_size, page_number)
    }

    /// Lists contracts and their predicates within a given DA block range.
    ///
    /// Returns each non-empty DA block number in the range alongside a
    /// `Vec<Contract>` containing the contracts appearing in that block.
    pub fn list_contracts(
        &self,
        block_range: Range<u64>,
    ) -> Result<Vec<(u64, Vec<Contract>)>, db::QueryError> {
        db::list_contracts(self, block_range)
    }

    /// Short-hand for constructing a transaction, providing it as an argument
    /// to the given function, then committing the transaction before returning.
    pub fn with_tx<T, E>(
        &mut self,
        f: impl FnOnce(&mut Transaction) -> Result<T, E>,
    ) -> Result<T, E>
    where
        E: From<rusqlite::Error>,
    {
        let mut tx = self.0.transaction()?;
        let res = f(&mut tx);
        tx.commit()?;
        res
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
        // By default, use an empty ID.
        Self::Memory(String::new())
    }
}

impl Default for Config {
    fn default() -> Self {
        // Here we use the number of available CPUs as a heuristic for a default
        // DB connection limit. This is because rusqlite `Connection` usage is
        // synchronous, and should be saturating the thread anyway.
        // TODO: Unsure if wasm-compatible? May want a feature for this?
        let conn_limit = num_cpus::get();
        let source = Source::default();
        Self { conn_limit, source }
    }
}

/// Initialise the connection pool from the given configuration.
pub(crate) fn new_conn_pool(conf: &Config) -> rusqlite::Result<AsyncConnectionPool> {
    AsyncConnectionPool::new(conf.conn_limit, || match &conf.source {
        Source::Memory(id) => new_mem_conn(id),
        Source::Path(p) => rusqlite::Connection::open(p),
    })
}

/// Create an in-memory connection with the given ID
fn new_mem_conn(id: &str) -> rusqlite::Result<rusqlite::Connection> {
    let flags = rusqlite::OpenFlags::default()
        | rusqlite::OpenFlags::SQLITE_OPEN_SHARED_CACHE
        | rusqlite::OpenFlags::SQLITE_OPEN_MEMORY;
    let conn_str = format!("file:{id}");
    rusqlite::Connection::open_with_flags(conn_str, flags)
}
