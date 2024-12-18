use essential_types::{ContentAddress, Word};
use thiserror::Error;

/// The result type for the relayer.
pub type Result<T> = std::result::Result<T, Error>;

/// The result type for internal errors.
pub(crate) type InternalResult<T> = std::result::Result<T, InternalError>;

/// Critical or recoverable errors that can occur in the relayer.
#[derive(Debug, Error)]
pub(crate) enum InternalError {
    /// A critical error occurred.
    #[error("a critical error occurred: {0}")]
    Critical(#[from] Error),
    /// A recoverable error occurred.
    #[error("a recoverable error occurred: {0}")]
    Recoverable(#[from] RecoverableError),
}

/// Alias for a critical error.
pub(crate) type CriticalError = Error;

/// An error occurred in the relayer that is not recoverable.
/// These causes the relayer to exit a spawned task.
#[derive(Debug, Error)]
pub enum Error {
    /// Failed to parse a server url.
    #[error("an error occurred when parsing the server url")]
    UrlParse,
    /// An overflow occurred.
    #[error("an overflow occurred when converting a number")]
    Overflow,
    /// A data sync error occurred.
    #[error("a data sync error occurred: {0}")]
    DataSyncFailed(#[from] DataSyncError),
    /// An error occurred while building the http client.
    #[error("an error occurred while building the http client: {0}")]
    HttpClientBuild(reqwest::Error),
    /// Failed to acquire then use a DB connection from the pool.
    #[error("failed to acquire or use a DB connection: {0}")]
    DbPoolRusqlite(#[from] essential_node_db::pool::AcquireThenRusqliteError),
}

/// An error that can be recovered from.
/// The stream will restart after logging a recoverable error.
#[derive(Debug, Error)]
pub(crate) enum RecoverableError {
    /// Stream from server failed.
    #[error("an error occurred in the stream from the server: {0}")]
    Stream(#[from] std::io::Error),
    /// Failed to make a request to the server.
    #[error("an error occurred in a request to the server: {0}")]
    BadServerResponse(reqwest::StatusCode),
    /// Http client error.
    #[error("an error occurred in the http client: {0}")]
    HttpClient(#[from] reqwest::Error),
    /// A new block was not sequentially after the last block.
    #[error("a new block was not sequentially after the last block. Got: {0}, expected: {1}")]
    NonSequentialBlock(Word, Word),
    /// The stream returned an error.
    #[error("the stream returned an error: {0}")]
    StreamError(String),
    /// A DB error occurred.
    #[error("a DB error occurred: {0}")]
    Rusqlite(rusqlite::Error),
}

#[derive(Debug, Error)]
/// An error occurred while syncing data.
pub enum DataSyncError {
    /// A fork was detected while syncing blocks.
    #[error(
        "While syncing blocks a fork was detected at block number {0}. Got: {1}, expected: {}", display_address(.2)
    )]
    Fork(Word, ContentAddress, Option<ContentAddress>),
}

fn display_address<T>(addr: &Option<T>) -> String
where
    T: core::fmt::Display,
{
    match addr {
        Some(addr) => format!("{}", addr),
        None => "None".to_string(),
    }
}

impl From<std::io::Error> for InternalError {
    fn from(e: std::io::Error) -> Self {
        InternalError::Recoverable(RecoverableError::Stream(e))
    }
}
