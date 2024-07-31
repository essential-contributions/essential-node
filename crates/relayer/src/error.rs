use essential_node_db::QueryError;
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
}
