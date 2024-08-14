use thiserror::Error;

use crate::db::AcquireThenQueryError;

#[derive(Debug, Error)]
pub(super) enum InternalError {
    #[error(transparent)]
    Recoverable(#[from] RecoverableError),
    #[error(transparent)]
    Critical(#[from] CriticalError),
}

#[derive(Debug, Error)]
pub enum RecoverableError {
    #[error("block {0} not found")]
    BlockNotFound(u64),
    #[error("could not read state")]
    ReadState(AcquireThenQueryError),
    #[error("failed to join handle")]
    Join(#[from] tokio::task::JoinError),
    #[error("failed to get last block")]
    LastProgress,
    #[error("A recoverable database error occurred: {0}")]
    Rusqlite(rusqlite::Error),
    #[error("failed to get new connection")]
    GetConnection(#[from] tokio::sync::AcquireError),
}

#[derive(Debug, Error)]
pub enum CriticalError {
    #[error("fork was found")]
    Fork,
    #[error("Critical database failure: {0}")]
    DatabaseFailed(#[from] rusqlite::Error),
    #[error("Critical database failure: {0}")]
    ReadState(#[from] AcquireThenQueryError),
}
