//! The Essential Node implementation.

use rusqlite_pool::tokio::AsyncConnectionPool;
use std::sync::Arc;
use thiserror::Error;

pub mod db;

/// An Essential `Node`.
pub struct Node {
    /// A fixed number of connections to the node's database.
    conn_pool: Arc<AsyncConnectionPool>,
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
        let conn_pool = Arc::new(db::new_conn_pool(&conf.db)?);
        let node = Self { conn_pool };

        // Ensure the DB tables exist.
        // TODO: Could do this here, or let application create tables?
        node.conn_pool()
            .try_acquire()
            .expect("all permits available upon creation")
            .create_tables()?;

        Ok(node)
    }

    /// Access to the node's DB connection pool.
    pub fn conn_pool(&self) -> db::ConnectionPool {
        db::ConnectionPool(self.conn_pool.clone())
    }

    /// Manually close the `Node` and handle the result.
    pub async fn close(self) -> Result<(), CloseError> {
        self.conn_pool.close().await?;
        Ok(())
    }
}
