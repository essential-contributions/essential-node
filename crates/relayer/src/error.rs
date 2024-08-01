use essential_node_db::QueryError;
use essential_types::ContentAddress;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    /// A DB error occurred.
    #[error("a DB error occurred: {0}")]
    Rusqlite(#[from] rusqlite::Error),
    /// Failed to join the db thread.
    #[error("an error occurred when joining the db writing thread: {0}")]
    DbWriteThreadFailed(#[from] tokio::task::JoinError),
    /// Failed to query the db.
    #[error("an error occurred when querying the db: {0}")]
    DbQueryFailed(#[from] QueryError),
    /// Stream from server failed.
    #[error("an error occurred in the stream from the server: {0}")]
    Stream(#[from] std::io::Error),
    /// Failed to make a request to the server.
    #[error("an error occurred in a request to the server: {0}")]
    BadServerResponse(reqwest::StatusCode),
    /// Failed to parse a server url.
    #[error("an error occurred when parsing the server url")]
    UrlParse,
    /// Http client error.
    #[error("an error occurred in the http client: {0}")]
    HttpClient(#[from] reqwest::Error),
    /// An overflow occurred.
    #[error("an overflow occurred when converting a number")]
    Overflow,
    /// A data sync error occurred.
    #[error("a data sync error occurred: {0}")]
    DataSyncFailed(#[from] DataSyncError),
    /// A new block was not sequentially after the last block.
    #[error("a new block was not sequentially after the last block. Got: {0}, expected: {1}")]
    NonSequentialBlock(u64, u64),
}

#[derive(Debug, Error)]
/// An error occurred while syncing data.
pub enum DataSyncError {
    /// A contract mismatch was found.
    #[error("While syncing a contract mismatch was found at: {0}. Got: {1}, expected: {}", display_address(.2))]
    ContractMismatch(u64, ContentAddress, Option<ContentAddress>),
    /// A fork was detected while syncing blocks.
    #[error(
        "While syncing a blocks a fork was detected at block number {0}. Got: {1}, expected: {}", display_address(.2)
    )]
    Fork(u64, ContentAddress, Option<ContentAddress>),
}

fn display_address(addr: &Option<ContentAddress>) -> String {
    match addr {
        Some(addr) => format!("{}", addr),
        None => "None".to_string(),
    }
}
