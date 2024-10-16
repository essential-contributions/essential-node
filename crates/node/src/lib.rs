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
pub use validate::validate;
pub use validate::validate_solution;
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
#[derive(Clone, Default)]
pub struct BlockTx(tokio::sync::watch::Sender<()>);

/// Wrapper around `watch::Receiver` to listen to new blocks.
///
/// This is used by [`db::subscribe_blocks`] stream.
#[derive(Clone)]
pub struct BlockRx(tokio::sync::watch::Receiver<()>);

/// Options for running the node.
#[derive(Clone, Debug)]
pub struct RunConfig {
    /// Node endpoint to sync blocks from.
    /// If `None` then the relayer stream will not run.
    pub relayer_source_endpoint: Option<String>,
    /// If `false` then the state derivation stream will not run.
    pub run_state_derivation: bool,
    /// If `false` then the validation stream will not run.
    pub run_validation: bool,
}

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
        conn.pragma_update(None, "journal_mode", "wal")?;
    };
    db::with_tx(&mut conn, |tx| essential_node_db::create_tables(tx))?;

    Ok(db)
}

/// Optionally run the relayer and state derivation and validation streams.
///
/// Relayer will sync blocks from the node API blocks stream to node database
/// and notify state derivation stream of new blocks via the shared watch channel.
///
/// Returns a [`Handle`] that can be used to close the streams.
/// The streams will continue to run until the handle is dropped.
pub fn run(
    conn_pool: ConnectionPool,
    conf: RunConfig,
    block_notify: BlockTx,
) -> Result<Handle, CriticalError> {
    let RunConfig {
        run_state_derivation,
        run_validation,
        relayer_source_endpoint,
    } = conf;

    // Run relayer.
    let relayer_handle = if let Some(relayer_source_endpoint) = relayer_source_endpoint {
        let relayer = Relayer::new(relayer_source_endpoint.as_str())?;
        Some(relayer.run(conn_pool.0.clone(), block_notify.0.clone())?)
    } else {
        None
    };

    // Run state derivation stream.
    let state_handle = if run_state_derivation {
        Some(state_derivation_stream(
            conn_pool.clone(),
            block_notify.new_listener().0,
        )?)
    } else {
        None
    };

    // Run validation stream.
    let validation_handle = if run_validation {
        Some(validation_stream(
            conn_pool.clone(),
            block_notify.new_listener().0,
        )?)
    } else {
        None
    };

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
