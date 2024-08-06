use essential_node_db::QueryError;
use thiserror::Error;

#[derive(Debug, Error)]
pub(super) enum InternalError {
    #[error(transparent)]
    Recoverable(#[from] RecoverableError),
    #[error(transparent)]
    Critical(#[from] CriticalError),
}

#[derive(Debug, Error)]
pub enum RecoverableError {
    #[error("block not found")]
    BlockNotFound,
    #[error("could not update state")]
    WriteStateError(#[from] rusqlite::Error),
    #[error("could not read state")]
    ReadStateError(#[from] QueryError),
    #[error("failed to join handle")]
    JoinError(#[from] tokio::task::JoinError),
    #[error("failed to get last block")]
    LastProgressError,
}

#[derive(Debug, Error)]
pub enum CriticalError {
    #[error("fork was found")]
    Fork,
    #[error("failed to get new connection")]
    GetConnection(#[from] rusqlite::Error),
}
