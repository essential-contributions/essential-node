use crate::db::AcquireThenQueryError;
use essential_node_db::QueryError;
use essential_types::{ContentAddress, PredicateAddress};
use thiserror::Error;

#[derive(Debug, Error)]
#[error("Connection pool creation failed: {0}")]
pub struct ConnPoolNewError(#[from] pub rusqlite::Error);

#[derive(Debug, Error)]
pub enum RunConfigError {
    #[error("at least one of relayer, state derivation or validation need to be enabled")]
    NoFeaturesEnabled,
}

#[derive(Debug, Error)]
pub enum NewHandleError {
    #[error("cannot create node run handle without any underlying handles")]
    NoHandles,
}

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
    #[error("predicate not in database: {0:?}")]
    PredicateNotFound(PredicateAddress),
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
    #[error(transparent)]
    NewHandle(#[from] NewHandleError),
}

#[derive(Debug, Error)]
pub enum StateReadError {
    #[error(transparent)]
    Query(#[from] QueryError),
    #[error("database connection pool closed")]
    DbPoolClosed(#[from] tokio::sync::AcquireError),
    #[error("recoverable database error {0}")]
    Rusqlite(#[from] rusqlite::Error),
    #[error("failed to join handle")]
    Join(#[from] tokio::task::JoinError),
    #[error("invalid key range")]
    KeyRangeError,
}

impl From<ValidationError> for InternalError {
    fn from(e: ValidationError) -> Self {
        match e {
            ValidationError::PredicateNotFound(addr) => {
                InternalError::Recoverable(RecoverableError::PredicateNotFound(addr))
            }
            ValidationError::Query(err) => InternalError::Recoverable(RecoverableError::Query(err)),
            ValidationError::DbPoolClosed(err) => {
                InternalError::Critical(CriticalError::DbPoolClosed(err))
            }
            ValidationError::Rusqlite(err) => {
                InternalError::Recoverable(RecoverableError::Rusqlite(err))
            }
            ValidationError::Join(err) => InternalError::Recoverable(RecoverableError::Join(err)),
        }
    }
}
