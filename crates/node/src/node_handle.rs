use crate::error::CriticalError;

pub struct Handle {
    relayer: essential_relayer::Handle,
    state: crate::state_handle::Handle<CriticalError>,
}

impl Handle {
    pub fn new(
        relayer: essential_relayer::Handle,
        state: crate::state_handle::Handle<CriticalError>,
    ) -> Self {
        Self { relayer, state }
    }

    pub async fn close(self) -> Result<(), CriticalError> {
        let Self { relayer, state } = self;
        state.close().await?;
        relayer.close().await?;
        Ok(())
    }

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
