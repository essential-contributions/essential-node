use crate::db::AcquireThenQueryError;
use essential_node_db::QueryError;
use essential_types::PredicateAddress;
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
    #[error("could not read state")]
    ReadState(AcquireThenQueryError),
    #[error("failed to join handle")]
    Join(#[from] tokio::task::JoinError),
    #[error("failed to get last block")]
    LastProgress,
    #[error("A recoverable database error occurred: {0}")]
    Rusqlite(rusqlite::Error),
    #[error("Validation failed: {0}")]
    Validation(#[from] ValidationError),
}

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("predicate not in database: {0:?}")]
    PredicateNotFound(PredicateAddress),
    #[error(transparent)]
    Query(#[from] QueryError),
}

#[derive(Debug, Error)]
pub enum CriticalError {
    #[error("fork was found")]
    Fork,
    #[error("Critical database failure: {0}")]
    DatabaseFailed(#[from] rusqlite::Error),
    #[error("Critical database failure: {0}")]
    ReadState(#[from] AcquireThenQueryError),
    #[error("database connection pool closed")]
    DbPoolClosed(#[from] tokio::sync::AcquireError),
    #[error(transparent)]
    Relayer(#[from] essential_relayer::Error),
}
