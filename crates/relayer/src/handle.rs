use crate::Result;
use tokio::sync::watch;

#[cfg(test)]
mod tests;

pub struct Handle {
    join_contracts: tokio::task::JoinHandle<Result<()>>,
    join_blocks: tokio::task::JoinHandle<Result<()>>,
    close: Close,
}

struct Close {
    close_contracts: watch::Sender<()>,
    close_blocks: watch::Sender<()>,
}

impl Handle {
    pub fn new(
        join_contracts: tokio::task::JoinHandle<Result<()>>,
        join_blocks: tokio::task::JoinHandle<Result<()>>,
        close_contracts: watch::Sender<()>,
        close_blocks: watch::Sender<()>,
    ) -> Self {
        Self {
            join_contracts,
            join_blocks,
            close: Close {
                close_contracts,
                close_blocks,
            },
        }
    }

    pub async fn close(self) -> Result<()> {
        let Self {
            join_contracts,
            join_blocks,
            close,
        } = self;
        close.close();
        let (br, cr) = futures::future::join(join_blocks, join_contracts).await;
        flatten_result(br).and(flatten_result(cr))
    }

    pub async fn join(self) -> Result<()> {
        let Self {
            join_contracts,
            join_blocks,
            close,
        } = self;
        let (r, f) = futures::future::select(join_blocks, join_contracts)
            .await
            .into_inner();
        close.close();
        let r2 = f.await;
        flatten_result(r).and(flatten_result(r2))
    }
}

impl Close {
    fn close(&self) {
        let _ = self.close_contracts.send(());
        let _ = self.close_blocks.send(());
    }
}

fn flatten_result(result: std::result::Result<Result<()>, tokio::task::JoinError>) -> Result<()> {
    match result {
        Ok(r) => r,
        Err(e) => {
            if e.is_panic() {
                std::panic::resume_unwind(e.into_panic())
            } else {
                Ok(())
            }
        }
    }
}

impl Drop for Close {
    fn drop(&mut self) {
        self.close();
    }
}
