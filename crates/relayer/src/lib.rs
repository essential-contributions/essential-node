#![warn(missing_docs)]
//! Relayer is a library that syncs data from a remote source into a local database.
//! The relayer syncs blocks.
//! There are notify channels to signal when new data has been synced.

use error::CriticalError;
pub use error::DataSyncError;
pub use error::Error;
use error::InternalError;
use error::InternalResult;
pub use error::Result;
use futures::StreamExt;
pub use handle::Handle;
use reqwest::{ClientBuilder, Url};
use rusqlite_pool::tokio::AsyncConnectionPool;
use std::future::Future;
use sync::stream_blocks;
use sync::sync_blocks;
use tokio::sync::watch;

mod error;
mod handle;
mod sync;
#[cfg(test)]
mod tests;

/// Relayer client that syncs data from a remote source into a local database.
#[derive(Debug, Clone)]
pub struct Relayer {
    endpoint: Url,
    client: reqwest::Client,
}

impl Relayer {
    /// Create a new relayer client from a node endpoint.
    pub fn new(endpoint: impl TryInto<Url>) -> Result<Self> {
        let endpoint = endpoint.try_into().map_err(|_| CriticalError::UrlParse)?;
        let client = ClientBuilder::new()
            .http2_prior_knowledge()
            .build()
            .map_err(CriticalError::HttpClientBuild)?;
        Ok(Self { endpoint, client })
    }

    /// Run the relayer client.
    /// This will sync blocks from the remote source into the local database.
    ///
    /// Streams are spawned and run in the background.
    /// A handle is returned that can be used to close or join the streams.
    ///
    /// The two watch channels are used to notify the caller when new data has been synced.
    pub fn run(self, conn: AsyncConnectionPool, new_block: watch::Sender<()>) -> Result<Handle> {
        // The blocks callback. This is a closure that will be called
        // every time the blocks stream is restarted.
        let blocks = move |shutdown: watch::Receiver<()>| {
            let conn = conn.clone();
            let relayer = self.clone();
            let notify = new_block.clone();
            async move {
                // Run the blocks stream
                relayer.run_blocks(conn, shutdown, notify).await
            }
        };

        run(blocks)
    }

    /// Run the blocks stream.
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    async fn run_blocks(
        &self,
        conn: AsyncConnectionPool,
        mut shutdown: watch::Receiver<()>,
        notify: watch::Sender<()>,
    ) -> InternalResult<()> {
        #[cfg(feature = "tracing")]
        tracing::info!("Stream starting");

        // Get the last progress that was made from the database.
        let progress = sync::get_block_progress(&conn).await?;

        // Create the stream of blocks.
        let stream = stream_blocks(&self.endpoint, &self.client, &progress).await?;

        // Setup a future that will close the stream when the shutdown signal is received.
        let close = async move {
            let _ = shutdown.changed().await;
            #[cfg(feature = "tracing")]
            tracing::info!("Shutting down blocks stream");
        };

        // Run the stream of blocks.
        sync_blocks(conn, &progress, notify, stream.take_until(close)).await
    }
}

/// Run the two streams spawned in the background.
///
/// Handles errors and returns a handle that can be used to close or join the streams.
///
/// Recoverable errors will be logged and the stream will be restarted.
/// Critical errors will cause the stream to end.
fn run<B, BFut>(mut blocks: B) -> Result<Handle>
where
    B: FnMut(watch::Receiver<()>) -> BFut + Send + 'static,
    BFut: Future<Output = InternalResult<()>> + Send,
{
    // Create a channels to signal the streams to shutdown.
    let (close_blocks, blocks_shutdown) = watch::channel(());

    let f = async move {
        loop {
            // Run the blocks stream callback
            let r = blocks(blocks_shutdown.clone()).await;
            match r {
                // Stream has ended, return from the task
                Ok(()) => return Ok(()),
                Err(e) => {
                    // Return error if it's critical or
                    // continue if it's recoverable
                    handle_error(e).await?;
                }
            }
        }
    };

    #[cfg(feature = "tracing")]
    use tracing::Instrument;

    #[cfg(feature = "tracing")]
    let f = f.instrument(tracing::info_span!("blocks_stream"));

    let join_blocks = tokio::spawn(f);

    Ok(Handle::new(join_blocks, close_blocks))
}

/// Exit on critical errors, log recoverable errors
async fn handle_error(e: InternalError) -> Result<()> {
    let e = map_recoverable_errors(e);
    match e {
        InternalError::Critical(e) => {
            #[cfg(feature = "tracing")]
            tracing::error!(
                "The relayer has encountered a critical error: {} and cannot recover.",
                e
            );
            Err(e)
        }
        #[cfg(feature = "tracing")]
        InternalError::Recoverable(e) => {
            // Slow down loop if source is unreachable
            if matches!(e, error::RecoverableError::HttpClient(_)) {
                // TODO: Make exponential backoff.
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
            tracing::error!("The relayer has encountered a recoverable error: {} and will now restart the stream.", e);

            Ok(())
        }
        #[cfg(not(feature = "tracing"))]
        InternalError::Recoverable(_) => Ok(()),
    }
}

/// Some critical error types contain variants that we should handle as recoverable errors.
/// This function maps those errors to recoverable errors.
fn map_recoverable_errors(e: InternalError) -> InternalError {
    // Map recoverable rusqlite errors to recoverable errors
    match e {
        InternalError::Critical(CriticalError::Rusqlite(e)) => match e {
            rus @ rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error {
                    code: rusqlite::ffi::ErrorCode::DatabaseLocked,
                    ..
                },
                _,
            )
            | rus @ rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error {
                    code: rusqlite::ffi::ErrorCode::DatabaseBusy,
                    ..
                },
                _,
            ) => InternalError::Recoverable(error::RecoverableError::Rusqlite(rus)),
            _ => InternalError::Critical(CriticalError::Rusqlite(e)),
        },
        _ => e,
    }
}
