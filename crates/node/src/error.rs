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
    #[error("block {0} not found")]
    BlockNotFound(u64),
    #[error("could not update state")]
    WriteState(#[from] rusqlite::Error),
    #[error("could not read state")]
    ReadState(#[from] QueryError),
    #[error("failed to join handle")]
    Join(#[from] tokio::task::JoinError),
    #[error("failed to get last block")]
    LastProgress,
    #[error("failed to get new connection")]
    Rusqlite(rusqlite::Error),
}

#[derive(Debug, Error)]
pub enum CriticalError {
    #[error("fork was found")]
    Fork,
    #[error("failed to get new connection")]
    GetConnection(#[from] rusqlite::Error),
}
