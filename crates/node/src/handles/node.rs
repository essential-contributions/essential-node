use crate::error::CriticalError;

/// Handle for closing or joining the relayer, state derivation and validation streams.
pub struct Handle {
    relayer: essential_relayer::Handle,
    state: crate::handles::state::Handle<CriticalError>,
    validation: crate::handles::validation::Handle<CriticalError>,
    new_block: tokio::sync::watch::Receiver<()>,
}

impl Handle {
    /// Create a new handle.
    pub(crate) fn new(
        relayer: essential_relayer::Handle,
        state: crate::handles::state::Handle<CriticalError>,
        validation: crate::handles::validation::Handle<CriticalError>,
        new_block: tokio::sync::watch::Receiver<()>,
    ) -> Self {
        Self {
            relayer,
            state,
            validation,
            new_block,
        }
    }

    /// Close the relayer, state derivation and validation streams.
    ///
    /// If this future is dropped then all three streams will be closed.
    pub async fn close(self) -> Result<(), CriticalError> {
        let Self {
            relayer,
            state,
            validation,
            ..
        } = self;
        state.close().await?;
        relayer.close().await?;
        validation.close().await?;
        Ok(())
    }

    /// Returns a new-block notification receiver.
    pub fn new_block(&self) -> tokio::sync::watch::Receiver<()> {
        self.new_block.clone()
    }

    /// Join the relayer, state derivation and validation streams.
    ///
    /// Waits for all three streams to finish.
    ///
    /// If this future is dropped then both streams will be closed.
    pub async fn join(self) -> Result<(), CriticalError> {
        let Self {
            relayer,
            state,
            validation,
            ..
        } = self;

        let relayer_future = relayer.join();
        tokio::pin!(relayer_future);

        let state_future = state.join();
        tokio::pin!(state_future);

        let validation_future = validation.join();
        tokio::pin!(validation_future);

        tokio::select! {
            r = relayer_future => {r?},
            r = state_future => {r?},
            r = validation_future => {r?},
        }

        Ok(())
    }
}
