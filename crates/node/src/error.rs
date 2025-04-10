use crate::db::{
    pool::{AcquireThenError, AcquireThenQueryError, AcquireThenRusqliteError},
    QueryError,
};
use essential_types::{predicate::PredicateDecodeError, ContentAddress, PredicateAddress};
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
    #[error("A recoverable database error occurred: {0}")]
    Rusqlite(rusqlite::Error),
    #[error("predicate not in database: {}", fmt_pred_addr(.0))]
    PredicateNotFound(PredicateAddress),
}

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error(transparent)]
    SolutionSetPredicates(#[from] SolutionSetPredicatesError),
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
    #[error("last progress cannot be none")]
    LastProgressNone,
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
pub enum SolutionSetPredicatesError {
    #[error("failed to acquire a connection from the pool: {0}")]
    Acquire(#[from] AcquireError),
    #[error("failed to query predicate with address `{}`: {1}", fmt_pred_addr(.0))]
    QueryPredicate(PredicateAddress, QueryPredicateError),
    #[error("solution attempts to solve an unregistered predicate {}", fmt_pred_addr(.0))]
    MissingPredicate(PredicateAddress),
}

#[derive(Debug, Error)]
pub enum PredicatesProgramsError {
    #[error("failed to acquire a connection from the pool: {0}")]
    Acquire(#[from] AcquireError),
    #[error("failed to query program with address {0}: {1}")]
    QueryProgram(ContentAddress, QueryProgramError),
    #[error("predicate contains an unregistered program {0}")]
    MissingProgram(ContentAddress),
}

#[derive(Debug, Error)]
pub enum QueryPredicateError {
    #[error(transparent)]
    Query(#[from] QueryError),
    #[error("the queried predicate is missing the word that encodes its length")]
    MissingLenBytes,
    #[error("the queried predicate length was invalid")]
    InvalidLenBytes,
    #[error("failed to decode the queried predicate: {0:?}")]
    Decode(#[from] PredicateDecodeError),
}

#[derive(Debug, Error)]
pub enum QueryProgramError {
    #[error(transparent)]
    Query(#[from] QueryError),
    #[error("the queried program is missing the word that encodes its length")]
    MissingLenBytes,
    #[error("the queried predicate length was invalid")]
    InvalidLenBytes,
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

impl From<SolutionSetPredicatesError> for InternalError {
    fn from(e: SolutionSetPredicatesError) -> Self {
        match e {
            SolutionSetPredicatesError::Acquire(err) => {
                InternalError::Critical(CriticalError::DbPoolClosed(err))
            }
            SolutionSetPredicatesError::MissingPredicate(addr) => {
                InternalError::Recoverable(RecoverableError::PredicateNotFound(addr))
            }
            SolutionSetPredicatesError::QueryPredicate(addr, err) => {
                InternalError::Recoverable(RecoverableError::QueryPredicate(addr, err))
            }
        }
    }
}

impl From<ValidationError> for InternalError {
    fn from(e: ValidationError) -> Self {
        match e {
            ValidationError::SolutionSetPredicates(err) => err.into(),
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

impl From<AcquireThenError<StateReadError>> for StateReadError {
    fn from(error: AcquireThenError<StateReadError>) -> Self {
        match error {
            AcquireThenError::Acquire(err) => StateReadError::DbPoolClosed(err),
            AcquireThenError::Inner(err) => err,
            AcquireThenError::Join(err) => StateReadError::Join(err),
        }
    }
}

impl From<AcquireThenError<ValidationError>> for InternalError {
    fn from(error: AcquireThenError<ValidationError>) -> Self {
        match error {
            AcquireThenError::Acquire(err) => {
                InternalError::Critical(CriticalError::DbPoolClosed(err))
            }
            AcquireThenError::Inner(err) => err.into(),
            AcquireThenError::Join(err) => InternalError::Recoverable(RecoverableError::Join(err)),
        }
    }
}

fn fmt_pred_addr(addr: &PredicateAddress) -> String {
    format!(
        "(contract: {}, predicate: {})",
        addr.contract, addr.predicate
    )
}
