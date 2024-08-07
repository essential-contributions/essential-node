//! The Essential Node implementation.

use thiserror::Error;

pub mod db;

/// An Essential `Node`.
pub struct Node {
    /// A fixed number of connections to the node's database.
    conn_pool: db::ConnectionPool,
}

/// All configuration options for a `Node` instance.
#[derive(Clone, Debug, Default)]
pub struct Config {
    /// Configuration related to the database.
    pub db: db::Config,
}

/// The result of manually closing the `Node`.
pub struct CloseResult {
    /// The result of closing each connection.
    pub conns: Vec<Result<(), (rusqlite::Connection, rusqlite::Error)>>,
}

/// Node creation failure.
#[derive(Debug, Error)]
#[error("`Node` creation failed: {0}")]
pub struct NewError(#[from] pub rusqlite::Error);

/// Node closure failure.
#[derive(Debug, Error)]
#[error("`Node` failed to close: {0}")]
pub struct CloseError(#[from] pub rusqlite_pool::tokio::AsyncCloseError);

impl Node {
    /// Create a new `Node` instance from the given configuration.
    pub fn new(conf: &Config) -> Result<Self, NewError> {
        let conn_pool = db::ConnectionPool::new(&conf.db)?;

        // Create the tables.
        let mut conn = conn_pool.try_acquire().expect("all permits available");
        db::with_tx(&mut conn, |tx| essential_node_db::create_tables(tx))?;

        Ok(Self { conn_pool })
    }

    /// Access to the node's DB connection pool and in turn, DB-access-related methods.
    pub fn db(&self) -> db::ConnectionPool {
        self.conn_pool.clone()
    }

    /// Manually close the `Node` and handle the result.
    ///
    /// This will signal closure to all [`db::ConnectionHandle`]s via the
    /// connection pool's semaphore, wait for them to be dropped and their inner
    /// [`rusqlite::Connection`]s to be closed before returning.
    pub async fn close(self) -> Result<(), CloseError> {
        self.conn_pool.0.close().await?;
        Ok(())
    }
}
