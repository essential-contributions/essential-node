#![deny(missing_docs)]
//! The Essential node implementation.
//!
//! The primary API for accessing blocks and contracts is provided via the
//! [`ConnectionPool`] type, accessible via the [`db`] function.
//!
//! The node, via the [`run`] function:
//! - Runs the relayer stream and syncs blocks.
//! - Performs validation.

use error::{BigBangError, CriticalError};
pub use essential_node_db as db;
use essential_node_types::{block_notify::BlockTx, BigBang};
use essential_relayer::Relayer;
use essential_types::ContentAddress;
pub use handles::node::Handle;
pub use validate::validate_dry_run;
pub use validate::validate_solution_dry_run;
use validation::validation_stream;

mod error;
mod handles;
#[cfg(any(feature = "test-utils", test))]
#[allow(missing_docs)]
pub mod test_utils;
pub mod validate;
mod validation;

/// Options for running the node.
#[derive(Clone, Debug)]
pub struct RunConfig {
    /// Node endpoint to sync blocks from.
    /// If `None` then the relayer stream will not run.
    pub relayer_source_endpoint: Option<String>,
    /// If `false` then the validation stream will not run.
    pub run_validation: bool,
}

/// Ensures that a big bang block exists in the DB for the given `BigBang` configuration.
///
/// If no block exists with `block_number` `0`, this inserts the big bang block.
///
/// If a block already exists with `block_number` `0`, this validates that its [`ContentAddress`]
/// matches the `ContentAddress` of the `Block` returned from [`BigBang::block`].
///
/// If validation has not yet begun, this initializes progress to begin from the big bang `Block`.
///
/// Returns the `ContentAddress` of the big bang `Block`.
#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
pub async fn ensure_big_bang_block(
    conn_pool: &db::ConnectionPool,
    big_bang: &BigBang,
) -> Result<ContentAddress, BigBangError> {
    let bb_block = big_bang.block();
    let bb_block_ca = essential_hash::content_addr(&bb_block);

    #[cfg(feature = "tracing")]
    tracing::debug!("Big Bang Block CA: {bb_block_ca}");

    // List out the first block.
    match conn_pool.list_blocks(0..1).await?.into_iter().next() {
        // If no block at block `0` exists, insert and "finalize" the big bang block.
        None => {
            #[cfg(feature = "tracing")]
            tracing::debug!("Big Bang Block not found - inserting into DB");
            let bbb_ca = bb_block_ca.clone();
            conn_pool
                .acquire_then(|conn| {
                    db::with_tx(conn, move |tx| {
                        db::insert_block(tx, &bb_block)?;
                        db::finalize_block(tx, &bbb_ca)?;
                        Ok::<_, rusqlite::Error>(())
                    })
                })
                .await?;
        }
        // If a block already exists, ensure its the big bang block we expect.
        Some(block) => {
            let ca = essential_hash::content_addr(&block);
            if ca != bb_block_ca {
                return Err(BigBangError::UnexpectedBlock {
                    expected: bb_block_ca,
                    found: ca,
                });
            }
            #[cfg(feature = "tracing")]
            tracing::debug!("Big Bang Block already exists");
        }
    }

    // If validation has not yet begun, ensure it begins from the big bang block.
    if conn_pool.get_validation_progress().await?.is_none() {
        #[cfg(feature = "tracing")]
        tracing::debug!("Starting validation progress at Big Bang Block CA");
        conn_pool
            .update_validation_progress(bb_block_ca.clone())
            .await?;
    }

    Ok(bb_block_ca)
}

/// Optionally run the relayer and validation stream.
///
/// Relayer will sync blocks from the node API blocks stream to node database
/// and notify validation stream of new blocks via the shared watch channel.
///
/// Returns a [`Handle`] that can be used to close the streams.
/// The streams will continue to run until the handle is dropped.
pub fn run(
    conn_pool: db::ConnectionPool,
    conf: RunConfig,
    contract_registry: ContentAddress,
    program_registry: ContentAddress,
    block_notify: BlockTx,
) -> Result<Handle, CriticalError> {
    let RunConfig {
        run_validation,
        relayer_source_endpoint,
    } = conf;

    // Run relayer.
    let relayer_handle = if let Some(relayer_source_endpoint) = relayer_source_endpoint {
        let relayer = Relayer::new(relayer_source_endpoint.as_str())?;
        Some(relayer.run(conn_pool.clone(), block_notify.clone())?)
    } else {
        None
    };

    // Run validation stream.
    let validation_handle = if run_validation {
        Some(validation_stream(
            conn_pool.clone(),
            contract_registry,
            program_registry,
            block_notify.new_listener(),
        )?)
    } else {
        None
    };

    Ok(Handle::new(relayer_handle, validation_handle))
}
