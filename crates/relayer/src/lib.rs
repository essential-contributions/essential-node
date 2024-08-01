use futures::StreamExt;
use reqwest::{ClientBuilder, Url};

pub use error::DataSyncError;
pub use error::Error;
pub use error::Result;
use sync::stream_blocks;
use sync::stream_contracts;
use sync::sync_blocks;
use sync::sync_contracts;
use sync::WithConn;
use tokio::sync::oneshot;
use tokio::sync::watch;

mod error;
mod sync;

#[derive(Debug, Clone)]
pub struct Relayer {
    endpoint: Url,
    client: reqwest::Client,
}

pub struct Handle {
    close_contracts: Option<oneshot::Sender<()>>,
    close_blocks: Option<oneshot::Sender<()>>,
    join_contracts: Option<tokio::task::JoinHandle<Result<()>>>,
    join_blocks: Option<tokio::task::JoinHandle<Result<()>>>,
}

impl Relayer {
    pub fn new(endpoint: impl TryInto<Url>) -> Result<Self> {
        let endpoint = endpoint.try_into().map_err(|_| Error::UrlParse)?;
        let client = ClientBuilder::new().http2_prior_knowledge().build()?;
        Ok(Self { endpoint, client })
    }

    pub fn run(
        self,
        contracts_conn: rusqlite::Connection,
        blocks_conn: rusqlite::Connection,
        notify: watch::Sender<()>,
    ) -> Result<Handle> {
        let (close_contracts, contracts_shutdown) = oneshot::channel();
        let (close_blocks, blocks_shutdown) = oneshot::channel();
        let relayer = self.clone();
        let join_contracts = tokio::spawn(async move {
            relayer
                .run_contracts(contracts_conn, contracts_shutdown)
                .await
        });
        let join_blocks =
            tokio::spawn(
                async move { self.run_blocks(blocks_conn, blocks_shutdown, notify).await },
            );
        Ok(Handle {
            close_contracts: Some(close_contracts),
            close_blocks: Some(close_blocks),
            join_contracts: Some(join_contracts),
            join_blocks: Some(join_blocks),
        })
    }

    async fn run_contracts(
        &self,
        conn: rusqlite::Connection,
        shutdown: oneshot::Receiver<()>,
    ) -> Result<()> {
        let WithConn {
            conn,
            value: progress,
        } = sync::get_contract_progress(conn).await?;
        sync::check_for_contract_mismatch(&self.endpoint, &self.client, &progress).await?;
        let l2_block_number = progress.as_ref().map(|p| p.l2_block_number);
        let stream = stream_contracts(&self.endpoint, &self.client, progress).await?;
        sync_contracts(conn, l2_block_number, stream.take_until(shutdown)).await
    }

    async fn run_blocks(
        &self,
        conn: rusqlite::Connection,
        shutdown: oneshot::Receiver<()>,
        notify: watch::Sender<()>,
    ) -> Result<()> {
        let WithConn {
            conn,
            value: progress,
        } = sync::get_block_progress(conn).await?;
        sync::check_for_block_fork(&self.endpoint, &self.client, &progress).await?;
        let last_block_number = progress.as_ref().map(|p| p.last_block_number);
        let stream = stream_blocks(&self.endpoint, &self.client, progress).await?;
        sync_blocks(conn, last_block_number, notify, stream.take_until(shutdown)).await
    }
}

impl Handle {
    pub async fn close(mut self) -> Result<()> {
        let Some((close_contracts, join_contracts)) = self
            .close_contracts
            .take()
            .and_then(|tx| Some((tx, self.join_contracts.take()?)))
        else {
            return Ok(());
        };

        let Some((close_blocks, join_blocks)) = self
            .close_blocks
            .take()
            .and_then(|tx| Some((tx, self.join_blocks.take()?)))
        else {
            return Ok(());
        };
        let close_con_err = close_contracts.send(()).is_err();
        let close_block_err = close_blocks.send(()).is_err();
        if close_con_err || close_block_err {
            let cr = if join_contracts.is_finished() {
                match join_contracts.await {
                    Ok(r) => r,
                    Err(_) => Ok(()),
                }
            } else {
                Ok(())
            };
            let br = if join_blocks.is_finished() {
                match join_blocks.await {
                    Ok(r) => r,
                    Err(_) => Ok(()),
                }
            } else {
                Ok(())
            };
            return cr.and(br);
        }

        let Ok(cr) = join_contracts.await else {
            return Ok(());
        };
        let Ok(br) = join_blocks.await else {
            return Ok(());
        };
        cr.and(br)
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        if let Some(tx) = self.close_contracts.take() {
            let _ = tx.send(());
        }
    }
}
