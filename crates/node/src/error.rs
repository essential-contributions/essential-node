use crate::db::{AcquireThenQueryError, AcquireThenRusqliteError};
use essential_node_db::QueryError;
use essential_types::{predicate, ContentAddress, PredicateAddress};
use thiserror::Error;
use tokio::sync::AcquireError;

#[derive(Debug, Error)]
#[error("Connection pool creation failed: {0}")]
pub struct ConnPoolNewError(#[from] pub rusqlite::Error);

/// Errors that can occur when joining the node handle.
#[derive(Debug, Error)]
pub enum NodeHandleJoinError {
    /// The relayer stream joined with an error.
    #[error("the relayer stream returned with an error: {0}")]
    Relayer(essential_relayer::Error),
    /// The validation stream joined with an error.
    #[error("the validation stream returned with an error: {0}")]
    Validation(CriticalError),
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
    #[error("could not read state")]
    ReadState(AcquireThenQueryError),
    #[error(transparent)]
    Query(#[from] QueryError),
    #[error("failed to query predicate with address `{}`: {1}", fmt_pred_addr(.0))]
    QueryPredicate(PredicateAddress, QueryPredicateError),
    #[error("failed to join handle")]
    Join(#[from] tokio::task::JoinError),
    #[error("failed to get last block")]
    LastProgress(#[from] AcquireThenQueryError),
    #[error("last progress cannot be none")]
    LastProgressNone,
    #[error("A recoverable database error occurred: {0}")]
    Rusqlite(rusqlite::Error),
    #[error("predicate not in database: {}", fmt_pred_addr(.0))]
    PredicateNotFound(PredicateAddress),
}

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error(transparent)]
    SolutionPredicates(#[from] SolutionPredicatesError),
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
    #[error("Critical error getting next block: {0}")]
    GetNextBlock(AcquireThenQueryError),
    #[error("database connection pool closed")]
    DbPoolClosed(#[from] tokio::sync::AcquireError),
    #[error(transparent)]
    Relayer(#[from] essential_relayer::Error),
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

#[derive(Debug, Error)]
pub enum SolutionPredicatesError {
    #[error("failed to acquire a connection from the pool: {0}")]
    Acquire(#[from] AcquireError),
    #[error("failed to query predicate with address `{}`: {1}", fmt_pred_addr(.0))]
    QueryPredicate(PredicateAddress, QueryPredicateError),
    #[error("solution attempts to solve an unregistered predicate {}", fmt_pred_addr(.0))]
    MissingPredicate(PredicateAddress),
}

#[derive(Debug, Error)]
pub enum QueryPredicateError {
    #[error(transparent)]
    Query(#[from] QueryError),
    #[error("the queried predicate is missing the word that encodes its length")]
    MissingLenBytes,
    #[error("the queried predicate length was invalid")]
    InvalidLenBytes,
    #[error("failed to decode the queried predicate: {0}")]
    Decode(#[from] predicate::header::DecodeError),
}

/// An error occurred while inserting of checking the big bang block.
#[derive(Debug, Error)]
pub enum BigBangError {
    /// Failed to query the DB via the connection pool.
    #[error("failed to query the DB via the connection pool: {0}")]
    ListBlocks(#[from] AcquireThenQueryError),
    /// Failed to insert the big bang block into the DB via the connection pool.
    #[error("failed to insert the big bang `Block` into the DB via the connection pool: {0}")]
    InsertBlock(#[from] AcquireThenRusqliteError),
    /// A block already exists at block `0`, and its `ContentAddress` does not match that of the
    /// big bang `Block` implied by the `BigBang` configuration.
    #[error(
        "existing big bang block does not match configuration\n  \
        expected: {expected}\n  \
        found:    {found}"
    )]
    UnexpectedBlock {
        expected: ContentAddress,
        found: ContentAddress,
    },
}

impl From<SolutionPredicatesError> for InternalError {
    fn from(e: SolutionPredicatesError) -> Self {
        match e {
            SolutionPredicatesError::Acquire(err) => {
                InternalError::Critical(CriticalError::DbPoolClosed(err))
            }
            SolutionPredicatesError::MissingPredicate(addr) => {
                InternalError::Recoverable(RecoverableError::PredicateNotFound(addr))
            }
            SolutionPredicatesError::QueryPredicate(addr, err) => {
                InternalError::Recoverable(RecoverableError::QueryPredicate(addr, err))
            }
        }
    }
}

impl From<ValidationError> for InternalError {
    fn from(e: ValidationError) -> Self {
        match e {
            ValidationError::SolutionPredicates(err) => err.into(),
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

fn fmt_pred_addr(addr: &PredicateAddress) -> String {
    format!(
        "(contract: {}, predicate: {})",
        addr.contract, addr.predicate
    )
}
