use crate::error::{CriticalError, NodeHandleJoinError};

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
    ) -> Self {
        Self {
            relayer,
            state,
            validation,
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
    pub async fn join(self) -> Result<(), NodeHandleJoinError> {
        let Self {
            relayer,
            state,
            validation,
        } = self;

        let relayer_future = async move {
            if let Some(relayer) = relayer {
                relayer.join().await.map_err(NodeHandleJoinError::Relayer)
            } else {
                Ok(())
            }
        };
        tokio::pin!(relayer_future);

        let state_future = async move {
            if let Some(state) = state {
                state
                    .join()
                    .await
                    .map_err(NodeHandleJoinError::StateDerivation)
            } else {
                Ok(())
            }
        };
        tokio::pin!(state_future);

        let validation_future = async move {
            if let Some(validation) = validation {
                validation
                    .join()
                    .await
                    .map_err(NodeHandleJoinError::Validation)
            } else {
                Ok(())
            }
        };
        tokio::pin!(validation_future);

        // Wait for all to successfully complete, or return early if one errors.
        tokio::try_join!(relayer_future, state_future, validation_future)?;
        Ok(())
    }
}
