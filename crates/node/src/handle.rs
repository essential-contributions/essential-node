use tokio::sync::watch;

/// Handle for joining or closing the stream.
pub struct Handle<E> {
    join: tokio::task::JoinHandle<Result<(), E>>,
    close: Close,
}

struct Close {
    close: watch::Sender<()>,
}

impl<E> Handle<E> {
    pub fn new(join: tokio::task::JoinHandle<Result<(), E>>, close: watch::Sender<()>) -> Self {
        Self {
            join,
            close: Close { close },
        }
    }

    pub async fn close(self) -> Result<(), E> {
        let _ = self.close.close.send(());
        flatten_result(self.join.await)
    }

    pub async fn join(self) -> Result<(), E> {
        flatten_result(self.join.await)
    }
}

/// Flatten the result of a join handle into the relayer result.
fn flatten_result<E>(
    result: std::result::Result<Result<(), E>, tokio::task::JoinError>,
) -> Result<(), E> {
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
        let _ = self.close.send(());
    }
}
