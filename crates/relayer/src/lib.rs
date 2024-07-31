use futures::StreamExt;
use reqwest::{ClientBuilder, Url};

pub use error::Error;
pub use error::Result;
use sync::stream_contracts;
use sync::sync_contracts;
use sync::WithConn;
use tokio::sync::oneshot;

mod error;
mod sync;

pub struct Relayer {
    endpoint: Url,
    client: reqwest::Client,
}

pub struct Handle {
    close: Option<oneshot::Sender<()>>,
    join: Option<tokio::task::JoinHandle<Result<()>>>,
}

impl Relayer {
    pub fn new(endpoint: impl TryInto<Url>) -> Result<Self> {
        let endpoint = endpoint.try_into().map_err(|_| Error::UrlParse)?;
        let client = ClientBuilder::new().http2_prior_knowledge().build()?;
        Ok(Self { endpoint, client })
    }

    pub fn run(self, conn: rusqlite::Connection) -> Result<Handle> {
        let (tx, rx) = oneshot::channel();
        let jh = tokio::spawn(async move {
            let WithConn {
                conn,
                value: progress,
            } = sync::get_contract_progress(conn).await?;
            let logical_clock = progress.as_ref().map(|p| p.logical_clock).unwrap_or(0);
            let stream = stream_contracts(&self.endpoint, &self.client, progress).await?;
            sync_contracts(conn, logical_clock, stream.take_until(rx)).await
        });
        Ok(Handle {
            close: Some(tx),
            join: Some(jh),
        })
    }
}

impl Handle {
    pub async fn close(mut self) -> Result<()> {
        let Some((tx, jh)) = self
            .close
            .take()
            .and_then(|tx| Some((tx, self.join.take()?)))
        else {
            return Ok(());
        };
        if tx.send(()).is_err() {
            return Ok(());
        }
        let Ok(result) = jh.await else {
            return Ok(());
        };
        result
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        if let Some(tx) = self.close.take() {
            let _ = tx.send(());
        }
    }
}
