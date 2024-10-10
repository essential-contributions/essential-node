#![deny(missing_docs)]
//! The Essential node implementation.
//!
//! The primary API for accessing blocks and contracts is provided via the
//! [`ConnectionPool`] type, accessible via the [`db`] function.
//!
//! The node, via the [`run`] function:
//! - Runs the relayer stream and syncs blocks.
//! - Derives state from the synced blocks.
//! - Performs validation.

use db::ConnectionPool;
use error::{ConnPoolNewError, CriticalError};
use essential_relayer::Relayer;
pub use handles::node::Handle;
use state_derivation::state_derivation_stream;
use validation::validation_stream;

pub mod db;
mod error;
mod handles;
mod state_derivation;
#[cfg(any(feature = "test-utils", test))]
#[allow(missing_docs)]
pub mod test_utils;
mod validate;
mod validation;

/// Wrapper around `watch::Sender` to notify of new blocks.
///
/// This is used by `essential-builder` to notify `essential-relayer`
/// and by `essential-relayer` to notify [`state_derivation`] and [`validation`] streams.
#[derive(Default)]
pub struct BlockTx(tokio::sync::watch::Sender<()>);

/// Wrapper around `watch::Receiver` to listen to new blocks.
///
/// This is used by [`db::subscribe_blocks`] stream.
#[derive(Clone)]
pub struct BlockRx(tokio::sync::watch::Receiver<()>);

/// Create a new `ConnectionPool` from the given configuration.
///
/// Upon construction, the node's database tables are created if they have
/// not already been created.
///
/// ## Example
///
/// ```rust
/// # use essential_node::{BlockTx, db::Config, db, run};
/// # #[tokio::main]
/// # async fn main() {
/// let conf = Config::default();
/// let db = essential_node::db(&conf).unwrap();
/// for block in db.list_blocks(0..100).await.unwrap() {
///     println!("Block: {block:?}");
/// }
/// # }
/// ```

pub fn db(conf: &db::Config) -> Result<ConnectionPool, ConnPoolNewError> {
    // Initialize the connection pool.
    let db = db::ConnectionPool::new(conf)?;

    // Create the tables.
    let mut conn = db.try_acquire().expect("all permits available");
    if let db::Source::Path(_) = conf.source {
        conn.pragma_update(None, "journal_mode", "WAL")?;
    };
    db::with_tx(&mut conn, |tx| essential_node_db::create_tables(tx))?;

    Ok(db)
}

/// Run the relayer and state derivation and validation streams.
///
/// Relayer will sync blocks from the node API blocks stream to node database
/// and notify state derivation stream of new blocks via the shared watch channel.
///
/// Returns a [`Handle`] that can be used to close the two streams.
/// The streams will continue to run until the handle is dropped.
pub fn run(
    conn_pool: ConnectionPool,
    block_notify: BlockTx,
    node_endpoint: String,
) -> Result<Handle, CriticalError> {
    // Run relayer.
    let relayer = Relayer::new(node_endpoint.as_str())?;
    let relayer_handle = relayer.run(conn_pool.0.clone(), block_notify.0.clone())?;

    // Run state derivation stream.
    let state_handle = state_derivation_stream(conn_pool.clone(), block_notify.new_listener().0)?;

    // Run validation stream.
    let validation_handle = validation_stream(conn_pool.clone(), block_notify.new_listener().0)?;

    Ok(Handle::new(relayer_handle, state_handle, validation_handle))
}

impl BlockTx {
    /// Create a new [`BlockTx`] to notify listeners of new blocks.
    pub fn new() -> Self {
        let (block_tx, _block_rx) = tokio::sync::watch::channel(());
        Self(block_tx)
    }

    /// Notify listeners that a new block has been received.
    ///
    /// Note this is best effort and will still send even if there are currently no listeners.
    pub fn notify(&self) {
        let _ = self.0.send(());
    }

    /// Create a new [`BlockRx`] to listen for new blocks.
    pub fn new_listener(&self) -> BlockRx {
        BlockRx(self.0.subscribe())
    }
}

impl BlockRx {
    /// Waits for a change notification.
    pub async fn changed(&mut self) -> Result<(), tokio::sync::watch::error::RecvError> {
        self.0.changed().await
    }
}
