//! The Essential Node implementation.
//!
//! The primary entry-point to the crate is the [`Node`] type.

use error::CriticalError;
use essential_relayer::Relayer;
pub use handles::node::Handle;
use rusqlite_pool::tokio::AsyncConnectionPool;
use state_derivation::state_derivation_stream;
use thiserror::Error;
use validation::validation_stream;

pub mod db;
mod error;
mod handles;
mod state_derivation;
#[cfg(any(feature = "test-utils", test))]
pub mod test_utils;
mod validate;
mod validation;

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
    conn_pools: ConnectionPools,
}

/// The node's DB connection pools, handling congestion and priority access to the sqlite DB.
struct ConnectionPools {
    /// A public connection pool providing DB access for downstream applications
    /// or library functionality like the node's API.
    public: db::ConnectionPool,
    /// A private internal connection pool, ensuring the relayer and state
    /// derivation have high-priority DB access in the case that the publicly
    /// exposed connection pool becomes congested.
    private: db::ConnectionPool,
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
        // Initialise the connections to the node's DB.
        let conn_pools = ConnectionPools::new(&conf.db)?;

        // Create the tables.
        let mut conn = conn_pools
            .private
            .try_acquire()
            .expect("all permits available");
        db::with_tx(&mut conn, |tx| essential_node_db::create_tables(tx))?;

        Ok(Self { conn_pools })
    }

    /// Access to the node's public DB connection pool and in turn,
    /// DB-access-related methods.
    ///
    /// Acquiring an instance of the [`db::ConnectionPool`] is cheap (equivalent
    /// to cloning an `Arc`).
    pub fn db(&self) -> db::ConnectionPool {
        self.conn_pools.public.clone()
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
        self.conn_pools.close()?;
        Ok(())
    }

    /// Run the `Node`.
    ///
    /// This method will start the relayer and state derivation streams.
    /// Relayer will sync blocks from the node API blocks stream to node database
    /// and notify state derivation stream of new blocks via the shared watch channel.
    ///
    /// Returns a [`Handle`] that can be used to close the two streams.
    /// The streams will continue to run until the handle is dropped.
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    pub fn run(&self, node_endpoint: String) -> Result<Handle, CriticalError> {
        // Run relayer.
        let (block_notify, new_block) = tokio::sync::watch::channel(());
        let relayer = Relayer::new(node_endpoint.as_str())?;
        let relayer_handle = relayer.run(self.conn_pools.private.0.clone(), block_notify)?;

        // Run state derivation stream.
        let state_handle =
            state_derivation_stream(self.conn_pools.private.clone(), new_block.clone())?;

        // Run validation stream.
        let validation_handle =
            validation_stream(self.conn_pools.private.clone(), new_block.clone())?;

        Ok(Handle::new(
            relayer_handle,
            state_handle,
            validation_handle,
            new_block,
        ))
    }
}

impl ConnectionPools {
    /// The node's API, relayer and state derivation connection pools.
    fn new(conf: &db::Config) -> rusqlite::Result<ConnectionPools> {
        // The private connections should get priority, and ideally be able
        // to roughly saturate the available CPUs while the relayer and state
        // streams catch up with head.
        let prv_conn_limit = num_cpus::get();
        let prv_conn_init = || db::new_conn(&conf.source);
        let private = db::ConnectionPool(AsyncConnectionPool::new(prv_conn_limit, prv_conn_init)?);
        let public = db::ConnectionPool::new(conf)?;
        Ok(Self { public, private })
    }

    /// Closes the inner connection pools, returning an error in the case that
    /// any of the queued connections fail to close.
    fn close(self) -> Result<(), ConnectionCloseErrors> {
        let prv_res = close_conn_pool(&self.private.0);
        let pub_res = close_conn_pool(&self.public.0);
        if let Err(mut err) = prv_res {
            if let Err(pub_err) = pub_res {
                err.0.extend(pub_err.0);
            }
            return Err(err);
        }
        Ok(())
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

/// Close a connection pool, returning a `ConnectionCloseErrors` in the case of any errors.
fn close_conn_pool(conn_pool: &AsyncConnectionPool) -> Result<(), ConnectionCloseErrors> {
    let res = conn_pool.close();
    let errs: Vec<_> = res.into_iter().filter_map(Result::err).collect();
    if !errs.is_empty() {
        return Err(ConnectionCloseErrors(errs));
    }
    Ok(())
}
