//! The Essential Node implementation.

use core::ops::Range;
use essential_node_db as db;
use essential_types::{
    contract::Contract, predicate::Predicate, solution::Solution, Block, ContentAddress, Key, Value,
};
use rusqlite::{Connection, Transaction};
use rusqlite_pool::tokio::{AsyncConnectionHandle, AsyncConnectionPool};
use std::{path::PathBuf, time::Duration};
use thiserror::Error;
use tokio::sync::{AcquireError, TryAcquireError};

/// An Essential `Node`.
pub struct Node {
    /// A fixed number of connections to the node's database.
    conn_pool: AsyncConnectionPool,
}

/// A temporary connection handle to a [`Node`]'s connection pool.
///
/// This is a thin wrapper around [`AsyncConnectionHandle`] providing short-hand
/// methods for common node DB access patterns.
///
/// A `NodeConnection` can be acquired via the [`Node::db_conn`] method.
pub struct NodeConnection(AsyncConnectionHandle);

/// All configuration options for a `Node` instance.
#[derive(Clone, Debug, Default)]
pub struct Config {
    /// Configuration related to the database.
    pub db: DbConfig,
}

/// Node configuration related to the database.
#[derive(Clone, Debug)]
pub struct DbConfig {
    /// The number of simultaneous connections to the database to maintain.
    pub conn_limit: usize,
    /// The source of the
    pub source: DbSource,
}

/// The source of the node's database.
#[derive(Clone, Debug)]
pub enum DbSource {
    /// Use an in-memory database using the given string as a unique ID.
    Memory(String),
    /// Use the database at the given path.
    Path(PathBuf),
}

/// Node creation failure.
#[derive(Debug, Error)]
#[error("`Node` creation failed: {0}")]
pub struct NewError(#[from] pub rusqlite::Error);

impl Node {
    /// Create a new `Node` instance from the given configuration.
    pub fn new(conf: &Config) -> Result<Self, NewError> {
        let conn_pool = new_conn_pool(&conf.db)?;
        let node = Self { conn_pool };

        // Ensure the DB tables exist.
        // TODO: Could do this here, or let application create tables?
        node.try_db_conn()
            .expect("all permits available and semaphore is not closed")
            .create_tables()?;

        Ok(node)
    }

    /// Acquire a temporary database [`NodeConnection`] from the inner pool.
    ///
    /// In the case that all connections are busy, waits for the first available
    /// connection.
    pub async fn db_conn(&self) -> Result<NodeConnection, AcquireError> {
        self.conn_pool.acquire().await.map(NodeConnection)
    }

    /// Attempt to synchronously acquire a temporary database [`NodeConnection`]
    /// from the inner pool.
    ///
    /// Returns `None` in the case that all database connections are busy.
    pub fn try_db_conn(&self) -> Result<NodeConnection, TryAcquireError> {
        self.conn_pool.try_acquire().map(NodeConnection)
    }
}

impl NodeConnection {
    // NOTE: Should we expose the table creation and insertion methods?

    /// Create all database tables.
    pub fn create_tables(self) -> rusqlite::Result<()> {
        self.access_tx(|tx| db::create_tables(tx))
    }

    /// Insert the given block into the `block` table and for each of its
    /// solutions, add a row into the `solution` and `block_solution` tables.
    pub fn insert_block(self, block: &Block) -> rusqlite::Result<()> {
        self.access_tx(|tx| db::insert_block(tx, block))
    }

    /// Insert the given contract into the `contract` table and for each of its
    /// predicates add entries to the `predicate` and `contract_predicate` tables.
    pub fn insert_contract(self, contract: &Contract, da_block_num: u64) -> rusqlite::Result<()> {
        self.access_tx(|tx| db::insert_contract(tx, contract, da_block_num))
    }

    /// Updates the state for a given contract content address and key.
    pub fn update_state(
        self,
        contract_ca: &ContentAddress,
        key: &Key,
        value: &Value,
    ) -> rusqlite::Result<()> {
        self.access(|conn| db::update_state(conn, contract_ca, key, value))
    }

    /// Deletes the state for a given contract content address and key.
    pub fn delete_state(self, contract_ca: &ContentAddress, key: &Key) -> rusqlite::Result<()> {
        self.access(|conn| db::delete_state(conn, contract_ca, key))
    }

    /// Fetches a contract by its content address.
    pub fn get_contract(self, ca: &ContentAddress) -> Result<Option<Contract>, db::QueryError> {
        self.access(|conn| db::get_contract(conn, ca))
    }

    /// Fetches a predicate by its predicate content address.
    pub fn get_predicate(
        self,
        predicate_ca: &ContentAddress,
    ) -> Result<Option<Predicate>, db::QueryError> {
        self.access(|conn| db::get_predicate(conn, predicate_ca))
    }

    /// Fetches a solution by its content address.
    pub fn get_solution(self, ca: &ContentAddress) -> Result<Option<Solution>, db::QueryError> {
        self.access(|conn| db::get_solution(conn, ca))
    }

    /// Fetches the state value for the given contract content address and key pair.
    pub fn query_state(
        self,
        contract_ca: &ContentAddress,
        key: &Key,
    ) -> Result<Option<Value>, db::QueryError> {
        self.access(|conn| db::get_state_value(conn, contract_ca, key))
    }

    /// Lists all blocks in the given range.
    pub fn list_blocks(self, block_range: Range<u64>) -> Result<Vec<Block>, db::QueryError> {
        self.access(|conn| db::list_blocks(conn, block_range))
    }

    /// Lists blocks and their solutions within a specific time range with pagination.
    pub fn list_blocks_by_time(
        self,
        range: Range<Duration>,
        page_size: i64,
        page_number: i64,
    ) -> Result<Vec<Block>, db::QueryError> {
        self.access(|conn| db::list_blocks_by_time(conn, range, page_size, page_number))
    }

    /// Lists contracts and their predicates within a given DA block range.
    ///
    /// Returns each non-empty DA block number in the range alongside a
    /// `Vec<Contract>` containing the contracts appearing in that block.
    pub fn list_contracts(
        self,
        block_range: Range<u64>,
    ) -> Result<Vec<(u64, Vec<Contract>)>, db::QueryError> {
        self.access(|conn| db::list_contracts(conn, block_range))
    }

    /// Calls [`ConnectionHandle::access`] on the inner connection handle with the given function.
    pub fn access<O>(mut self, f: impl FnOnce(&mut Connection) -> O) -> O {
        f(&mut self.0)
    }

    /// Short-hand for calling [`ConnectionHandle::access`] with a function that
    /// constructs a [`Transaction`], provides it to the given function and then
    /// commits the result.
    pub fn access_tx<T, E>(self, f: impl FnOnce(&mut Transaction) -> Result<T, E>) -> Result<T, E>
    where
        E: From<rusqlite::Error>,
    {
        self.access(|conn| {
            let mut tx = conn.transaction()?;
            let res = f(&mut tx);
            tx.commit()?;
            res
        })
    }
}

impl Default for DbSource {
    fn default() -> Self {
        // By default, use an empty ID.
        Self::Memory(String::new())
    }
}

impl Default for DbConfig {
    fn default() -> Self {
        // Here we use the number of available CPUs as a heuristic for a default
        // DB connection limit. This is because rusqlite `Connection` usage is
        // synchronous, and should be saturating the thread anyway.
        // TODO: Unsure if wasm-compatible? May want a feature for this?
        let conn_limit = num_cpus::get();
        let source = DbSource::default();
        Self { conn_limit, source }
    }
}

/// Initialise the connection pool from the given configuration.
fn new_conn_pool(conf: &DbConfig) -> rusqlite::Result<AsyncConnectionPool> {
    AsyncConnectionPool::new(conf.conn_limit, || match &conf.source {
        DbSource::Memory(id) => new_mem_conn(id),
        DbSource::Path(p) => rusqlite::Connection::open(p),
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
