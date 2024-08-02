use std::borrow::BorrowMut;
use std::future::Future;

use error::InternalError;
use error::InternalResult;
use futures::StreamExt;
use handle::Handle;
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

#[derive(Debug, Clone)]
pub struct Relayer {
    endpoint: Url,
    client: reqwest::Client,
}

pub trait GetConn {
    type Error;
    type Connection: BorrowMut<rusqlite::Connection> + Send + 'static;

    fn get(
        &self,
    ) -> impl Future<Output = std::result::Result<Self::Connection, Self::Error>> + Send;
}

impl Relayer {
    pub fn new(endpoint: impl TryInto<Url>) -> Result<Self> {
        let endpoint = endpoint.try_into().map_err(|_| CriticalError::UrlParse)?;
        let client = ClientBuilder::new()
            .http2_prior_knowledge()
            .build()
            .map_err(CriticalError::HttpClientBuild)?;
        Ok(Self { endpoint, client })
    }

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
        let contracts = move |shutdown: watch::Receiver<()>| {
            let conn = conn.clone();
            let relayer = relayer.clone();
            let notify = new_contract.clone();
            async move {
                let c = conn.get().await.map_err(CriticalError::from)?;
                relayer.run_contracts(c, shutdown, notify).await
            }
        };
        let blocks = move |shutdown: watch::Receiver<()>| {
            let conn = get_conn.clone();
            let relayer = self.clone();
            let notify = new_block.clone();
            async move {
                let c = conn.get().await.map_err(CriticalError::from)?;
                relayer.run_blocks(c, shutdown, notify).await
            }
        };
        run(contracts, blocks)
    }

    async fn run_contracts<C>(
        &self,
        conn: C,
        mut shutdown: watch::Receiver<()>,
        notify: watch::Sender<()>,
    ) -> InternalResult<()>
    where
        C: BorrowMut<rusqlite::Connection> + Send + 'static,
    {
        let WithConn {
            conn,
            value: progress,
        } = sync::get_contract_progress(conn).await?;
        sync::check_for_contract_mismatch(&self.endpoint, &self.client, &progress).await?;
        let l2_block_number = progress.as_ref().map(|p| p.l2_block_number);
        let stream = stream_contracts(&self.endpoint, &self.client, progress).await?;
        let close = async move {
            let _ = shutdown.changed().await;
        };
        sync_contracts(conn, l2_block_number, notify, stream.take_until(close)).await
    }

    async fn run_blocks<C>(
        &self,
        conn: C,
        mut shutdown: watch::Receiver<()>,
        notify: watch::Sender<()>,
    ) -> InternalResult<()>
    where
        C: BorrowMut<rusqlite::Connection> + Send + 'static,
    {
        let WithConn {
            conn,
            value: progress,
        } = sync::get_block_progress(conn).await?;
        sync::check_for_block_fork(&self.endpoint, &self.client, &progress).await?;
        let last_block_number = progress.as_ref().map(|p| p.last_block_number);
        let stream = stream_blocks(&self.endpoint, &self.client, progress).await?;
        let close = async move {
            let _ = shutdown.changed().await;
        };
        sync_blocks(conn, last_block_number, notify, stream.take_until(close)).await
    }
}

fn run<C, B, CFut, BFut>(mut contracts: C, mut blocks: B) -> Result<Handle>
where
    C: FnMut(watch::Receiver<()>) -> CFut + Send + 'static,
    CFut: Future<Output = InternalResult<()>> + Send,
    B: FnMut(watch::Receiver<()>) -> BFut + Send + 'static,
    BFut: Future<Output = InternalResult<()>> + Send,
{
    let (close_contracts, contracts_shutdown) = watch::channel(());
    let (close_blocks, blocks_shutdown) = watch::channel(());
    let join_contracts = tokio::spawn(async move {
        loop {
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
    });
    let join_blocks = tokio::spawn(async move {
        loop {
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
    });
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
        InternalError::Critical(e) => Err(e),
        InternalError::Recoverable(e) => {
            // TODO: Change to tracing
            eprintln!("Recoverable error: {}", e);
            Ok(())
        }
    }
}
