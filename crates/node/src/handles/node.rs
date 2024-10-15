use crate::error::{CriticalError, NewHandleError};

/// Handle for closing or joining the relayer, state derivation and validation streams.
pub struct Handle {
    relayer: Option<essential_relayer::Handle>,
    state: Option<crate::handles::state::Handle<CriticalError>>,
    validation: Option<crate::handles::validation::Handle<CriticalError>>,
}

impl Handle {
    /// Create a new handle.
    pub(crate) fn new(
        relayer: Option<essential_relayer::Handle>,
        state: Option<crate::handles::state::Handle<CriticalError>>,
        validation: Option<crate::handles::validation::Handle<CriticalError>>,
    ) -> Result<Self, NewHandleError> {
        if relayer.is_none() && state.is_none() && validation.is_none() {
            return Err(NewHandleError::NoHandles);
        }
        Ok(Self {
            relayer,
            state,
            validation,
        })
    }

    /// Close the relayer, state derivation and validation streams.
    ///
    /// If this future is dropped then all three streams will be closed.
    pub async fn close(self) -> Result<(), CriticalError> {
        let Self {
            relayer,
            state,
            validation,
        } = self;
        if let Some(state) = state {
            state.close().await?;
        }
        if let Some(relayer) = relayer {
            relayer.close().await?;
        }
        if let Some(validation) = validation {
            validation.close().await?;
        }
        Ok(())
    }

    /// Join the relayer, state derivation and validation streams.
    ///
    /// Does not close but waits for all three streams to finish.
    /// If any of the streams finish or error then all streams will be closed.
    ///
    /// If this future is dropped then both streams will be closed.
    pub async fn join(self) -> Result<(), CriticalError> {
        let Self {
            relayer,
            state,
            validation,
        } = self;
        let relayer_future = async move {
            if let Some(relayer) = relayer {
                relayer.join().await
            } else {
                Ok(())
            }
        };
        tokio::pin!(relayer_future);

        let state_future = async move {
            if let Some(state) = state {
                state.join().await
            } else {
                Ok(())
            }
        };
        tokio::pin!(state_future);

        let validation_future = async move {
            if let Some(validation) = validation {
                validation.join().await
            } else {
                Ok(())
            }
        };
        tokio::pin!(validation_future);

        tokio::select! {
            r = relayer_future => {r?},
            r = state_future => {r?},
            r = validation_future => {r?},
        }

        Ok(())
    }
}
