use crate::error::CriticalError;

/// Handle for closing or joining the relayer and state derivation streams.
pub struct Handle {
    relayer: essential_relayer::Handle,
    state: crate::handles::state::Handle<CriticalError>,
}

impl Handle {
    /// Create a new handle.
    pub(crate) fn new(
        relayer: essential_relayer::Handle,
        state: crate::handles::state::Handle<CriticalError>,
    ) -> Self {
        Self { relayer, state }
    }

    /// Close the relayer and state derivation streams.
    ///
    /// If this future is dropped then both streams will be closed.
    pub async fn close(self) -> Result<(), CriticalError> {
        let Self { relayer, state } = self;
        state.close().await?;
        relayer.close().await?;
        Ok(())
    }

    /// Join the relayer and state derivation streams.
    ///
    /// Waits for either stream to finish and awaits the other one.
    ///
    /// If this future is dropped then both streams will be closed.
    pub async fn join(self) -> Result<(), CriticalError> {
        let Self { relayer, state } = self;

        let relayer_future = relayer.join();
        tokio::pin!(relayer_future);

        let state_future = state.join();
        tokio::pin!(state_future);

        let f = futures::future::select(relayer_future, state_future).await;

        match f {
            futures::future::Either::Left((r, other)) => match r {
                Ok(()) => {
                    other.await?;
                }
                Err(e) => return Err(e.into()),
            },
            futures::future::Either::Right((r, other)) => match r {
                Ok(()) => {
                    other.await?;
                }
                Err(e) => return Err(e),
            },
        }

        Ok(())
    }
}
