use crate::Result;
use tokio::sync::watch;

#[cfg(test)]
mod tests;

/// Handle for closing or joining the relayer.
pub struct Handle {
    join_blocks: tokio::task::JoinHandle<Result<()>>,
    close: Close,
}

/// Struct which when dropped will close the relayer.
struct Close {
    close_blocks: watch::Sender<()>,
}

impl Handle {
    /// Create a new handle.
    pub fn new(
        join_blocks: tokio::task::JoinHandle<Result<()>>,
        close_blocks: watch::Sender<()>,
    ) -> Self {
        Self {
            join_blocks,
            close: Close { close_blocks },
        }
    }

    /// Close the Relayer streams and join them.
    ///
    /// If this future isn't polled the streams will continue to run.
    /// However, if the future is dropped the streams will be closed.
    pub async fn close(self) -> Result<()> {
        let Self { join_blocks, close } = self;
        // Close the streams.
        close.close();

        // Join both the streams.
        let br = join_blocks.await;

        // Flatten the results together.
        flatten_result(br)
    }

    /// Join the Relayer streams.
    ///
    /// This does not close the streams.
    /// Instead it waits for the streams to finish.
    ///
    /// However, if either stream finishes or errors then
    /// both streams are closed.
    ///
    /// If this future is dropped then both streams will close.
    pub async fn join(self) -> Result<()> {
        let Self { join_blocks, close } = self;
        let r = join_blocks.await;
        close.close();
        flatten_result(r)
    }
}

impl Close {
    fn close(&self) {
        let _ = self.close_blocks.send(());
    }
}

/// Flatten the result of a join handle into the relayer result.
fn flatten_result(result: std::result::Result<Result<()>, tokio::task::JoinError>) -> Result<()> {
    match result {
        // Joined successfully.
        // Return the result from the task.
        Ok(r) => r,
        Err(e) => {
            // If the task panicked then resume the panic.
            if e.is_panic() {
                std::panic::resume_unwind(e.into_panic())
            } else {
                // If the task was cancelled then we consider the stream
                // to successfully finished.
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
