#![warn(missing_docs)]
//! Relayer is a library that syncs data from a remote source into a local database.
//! The relayer syncs contracts and blocks.
//! There are notify channels to signal when new data has been synced.

use std::borrow::BorrowMut;
use std::future::Future;

use error::InternalError;
use error::InternalResult;
use futures::StreamExt;
pub use handle::Handle;
use reqwest::{ClientBuilder, Url};

use error::CriticalError;
pub use error::DataSyncError;
pub use error::Error;
pub use error::Result;
use sync::stream_blocks;
use sync::stream_contracts;
use sync::sync_blocks;
use sync::sync_contracts;
use sync::WithConn;
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

/// Trait for getting a connection to the database.
///
/// Connections are consumed by the relayer and are not returned.
/// However this should be fairly infrequent.
pub trait GetConn {
    /// The error type.
    type Error;
    /// The connection type.
    type Connection: BorrowMut<rusqlite::Connection> + Send + 'static;

    /// Get a connection to the database.
    fn get(
        &self,
    ) -> impl Future<Output = std::result::Result<Self::Connection, Self::Error>> + Send;
}

impl Relayer {
    /// Create a new relayer client from a essential-server endpoint url.
    pub fn new(endpoint: impl TryInto<Url>) -> Result<Self> {
        let endpoint = endpoint.try_into().map_err(|_| CriticalError::UrlParse)?;
        let client = ClientBuilder::new()
            .http2_prior_knowledge()
            .build()
            .map_err(CriticalError::HttpClientBuild)?;
        Ok(Self { endpoint, client })
    }

    /// Run the relayer client.
    /// This will sync contracts and blocks from the remote source into the local database.
    ///
    /// Streams are spawned and run in the background.
    /// A handle is returned that can be used to close or join the streams.
    ///
    /// The two watch channels are used to notify the caller when new data has been synced.
    pub fn run<C, E>(
        self,
        get_conn: C,
        new_contract: watch::Sender<()>,
        new_block: watch::Sender<()>,
    ) -> Result<Handle>
    where
        C: GetConn<Error = E> + Clone + Send + 'static,
        Error: From<E>,
    {
        let relayer = self.clone();
        let conn = get_conn.clone();

        // The contracts callback. This is a closure that will be called
        // every time the contracts stream is restarted.
        let contracts = move |shutdown: watch::Receiver<()>| {
            let conn = conn.clone();
            let relayer = relayer.clone();
            let notify = new_contract.clone();
            async move {
                // Get a new connection to the database
                let c = conn.get().await.map_err(CriticalError::from)?;

                // Run the contracts stream
                relayer.run_contracts(c, shutdown, notify).await
            }
        };

        // The blocks callback. This is a closure that will be called
        // every time the blocks stream is restarted.
        let blocks = move |shutdown: watch::Receiver<()>| {
            let conn = get_conn.clone();
            let relayer = self.clone();
            let notify = new_block.clone();
            async move {
                // Get a new connection to the database
                let c = conn.get().await.map_err(CriticalError::from)?;
                // Run the contracts stream
                relayer.run_blocks(c, shutdown, notify).await
            }
        };

        run(contracts, blocks)
    }

    /// Run the contracts stream.
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    async fn run_contracts<C>(
        &self,
        conn: C,
        mut shutdown: watch::Receiver<()>,
        notify: watch::Sender<()>,
    ) -> InternalResult<()>
    where
        C: BorrowMut<rusqlite::Connection> + Send + 'static,
    {
        // Get the last progress that was made from the database.
        let WithConn {
            conn,
            value: progress,
        } = sync::get_contract_progress(conn).await?;

        // Create the stream of contracts.
        let stream = stream_contracts(&self.endpoint, &self.client, &progress).await?;

        // Setup a future that will close the stream when the shutdown signal is received.
        let close = async move {
            let _ = shutdown.changed().await;
            #[cfg(feature = "tracing")]
            tracing::info!("Shutting down contract stream");
        };

        // Run the stream of contracts.
        sync_contracts(conn, &progress, notify, stream.take_until(close)).await
    }

    /// Run the blocks stream.
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    async fn run_blocks<C>(
        &self,
        conn: C,
        mut shutdown: watch::Receiver<()>,
        notify: watch::Sender<()>,
    ) -> InternalResult<()>
    where
        C: BorrowMut<rusqlite::Connection> + Send + 'static,
    {
        // Get the last progress that was made from the database.
        let WithConn {
            conn,
            value: progress,
        } = sync::get_block_progress(conn).await?;

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
fn run<C, B, CFut, BFut>(mut contracts: C, mut blocks: B) -> Result<Handle>
where
    C: FnMut(watch::Receiver<()>) -> CFut + Send + 'static,
    CFut: Future<Output = InternalResult<()>> + Send,
    B: FnMut(watch::Receiver<()>) -> BFut + Send + 'static,
    BFut: Future<Output = InternalResult<()>> + Send,
{
    // Create a channels to signal the streams to shutdown.
    let (close_contracts, contracts_shutdown) = watch::channel(());
    let (close_blocks, blocks_shutdown) = watch::channel(());

    let f = async move {
        loop {
            // Run the contracts stream callback
            let r = contracts(contracts_shutdown.clone()).await;
            match r {
                // Stream has ended, return from the task
                Ok(()) => return Ok(()),
                Err(e) => {
                    // Return error if it's critical or
                    // continue if it's recoverable
                    handle_error(e)?;
                }
            }
        }
    };

    #[cfg(feature = "tracing")]
    use tracing::Instrument;

    #[cfg(feature = "tracing")]
    let f = f.instrument(tracing::info_span!("contracts_stream"));

    let join_contracts = tokio::spawn(f);

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
                    handle_error(e)?;
                }
            }
        }
    };

    #[cfg(feature = "tracing")]
    let f = f.instrument(tracing::info_span!("blocks_stream"));

    let join_blocks = tokio::spawn(f);

    Ok(Handle::new(
        join_contracts,
        join_blocks,
        close_contracts,
        close_blocks,
    ))
}

/// Exit on critical errors, log recoverable errors
fn handle_error(e: InternalError) -> Result<()> {
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
            tracing::error!("The relayer has encountered a recoverable error: {} and will now restart the stream.", e);

            Ok(())
        }
        #[cfg(not(feature = "tracing"))]
        InternalError::Recoverable(_) => Ok(()),
    }
}
