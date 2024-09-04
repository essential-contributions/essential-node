use crate::db::AcquireThenQueryError;
use essential_check::solution::PredicatesError;
use essential_node_db::QueryError;
use essential_types::{ContentAddress, PredicateAddress};
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
    #[error("block 0 not found")]
    FirstBlockNotFound,
    #[error("block {0} not found")]
    BlockNotFound(ContentAddress),
    #[error("could not read state")]
    ReadState(AcquireThenQueryError),
    #[error(transparent)]
    Query(#[from] QueryError),
    #[error("failed to join handle")]
    Join(#[from] tokio::task::JoinError),
    #[error("failed to get last block")]
    LastProgress,
    #[error("A recoverable database error occurred: {0}")]
    Rusqlite(rusqlite::Error),
}

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("predicate not in database: {0:?}")]
    PredicateNotFound(PredicateAddress),
    #[error(transparent)]
    Query(#[from] QueryError),
    #[error("database connection pool closed")]
    DbPoolClosed(#[from] tokio::sync::AcquireError),
    #[error("recoverable database error {0}")]
    Rusqlite(#[from] rusqlite::Error),
    #[error(transparent)]
    Validation(#[from] PredicatesError<QueryError>),
    #[error("failed to join handle")]
    Join(#[from] tokio::task::JoinError),
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

impl From<ValidationError> for InternalError {
    fn from(e: ValidationError) -> Self {
        todo!("convert ValidationError to InternalError for validation stream")
    }
}
