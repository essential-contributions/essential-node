//! The Essential Node implementation.
//!
//! The primary entry-point to the crate is the [`Node`] type.

use error::CriticalError;
use essential_relayer::Relayer;
pub use node_handle::Handle;
use state::derive_state_stream;
use thiserror::Error;

pub mod db;
mod error;
mod node_handle;
mod state;
mod state_handle;
#[cfg(any(feature = "test-utils", test))]
pub mod test_utils;

/// The Essential `Node`.
///
/// The node is reponsible for:
///
/// - Managing and providing access to the database via its connection pool.
/// - (to-do) Running the relayer stream and syncing blocks.
/// - (to-do) Deriving state from the synced blocks.
/// - (to-do) Optionally performing validation.
///
/// The node's primary API for accessing blocks and contracts is provided via
/// its [`db::ConnectionPool`], accessible via the [`Node::db`] method.
///
/// ## Example
///
/// ```rust
/// # use essential_node::{Config, Node};
/// # #[tokio::main]
/// # async fn main() {
/// let conf = Config::default();
/// let node = Node::new(&conf).unwrap();
/// for block in node.db().list_blocks(0..100).await.unwrap() {
///     println!("Block: {block:?}");
/// }
/// # }
/// ```
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

/// Node creation failure.
#[derive(Debug, Error)]
#[error("`Node` creation failed: {0}")]
pub struct NewError(#[from] pub rusqlite::Error);

/// Node closure failure.
#[derive(Debug, Error)]
#[error("`Node` failed to close: {0}")]
pub struct CloseError(#[from] pub ConnectionCloseErrors);

/// One or more connections failed to close.
#[derive(Debug, Error)]
pub struct ConnectionCloseErrors(pub Vec<(rusqlite::Connection, rusqlite::Error)>);

impl Node {
    /// Create a new `Node` instance from the given configuration.
    ///
    /// Upon construction, the node's database tables are created if they have
    /// not already been created.
    pub fn new(conf: &Config) -> Result<Self, NewError> {
        let conn_pool = db::ConnectionPool::new(&conf.db)?;

        // Create the tables.
        let mut conn = conn_pool.try_acquire().expect("all permits available");
        db::with_tx(&mut conn, |tx| essential_node_db::create_tables(tx))?;

        Ok(Self { conn_pool })
    }

    /// Access to the node's DB connection pool and in turn, DB-access-related methods.
    ///
    /// Acquiring an instance of the [`db::ConnectionPool`] is cheap (equivalent
    /// to cloning an `Arc`).
    pub fn db(&self) -> db::ConnectionPool {
        self.conn_pool.clone()
    }

    /// Manually close the `Node` and handle the result.
    ///
    /// Closes the inner connection pool, returning an error in the case that
    /// any of the queued connections fail to close.
    ///
    /// Ensure all [`db::ConnectionHandle`]s are dropped before calling `close`
    /// to properly handle all connection results. Otherwise, connections not in
    /// the queue will be closed upon the last connection handle dropping.
    pub fn close(self) -> Result<(), CloseError> {
        let res = self.conn_pool.0.close();
        let errs: Vec<_> = res.into_iter().filter_map(Result::err).collect();
        if !errs.is_empty() {
            return Err(ConnectionCloseErrors(errs).into());
        }
        Ok(())
    }

    /// Run the `Node`.
    ///
    /// This method will start the relayer and state derivation streams.
    /// Relayer will sync contracts and blocks from the server to node database
    /// and notify state derivation stream of new blocks via the shared watch channel.
    ///
    /// Returns a [`Handle`] that can be used to close the two streams.
    /// The streams will continue to run until the handle is dropped.
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    pub async fn run(&self, server_address: String) -> Result<Handle, CriticalError> {
        // Run relayer.
        let (contract_notify, _new_contract) = tokio::sync::watch::channel(());
        let (block_notify, new_block) = tokio::sync::watch::channel(());
        let relayer = Relayer::new(server_address.as_str())?;
        let relayer_handle =
            relayer.run((*self.conn_pool.0).clone(), contract_notify, block_notify)?;

        // Run state derivation stream.
        let state_handle = derive_state_stream(self.conn_pool.clone(), new_block)?;

        Ok(Handle::new(relayer_handle, state_handle))
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
