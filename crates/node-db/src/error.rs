use thiserror::Error;

/// Any error that might occur during decoding of a type returned by the DB.
#[derive(Debug, Error)]
#[error("decoding failed due to postcard deserialization error: {0}")]
pub struct DecodeError(#[from] pub postcard::Error);

/// A database or decoding error returned by a query.
#[derive(Debug, Error)]
pub enum QueryError {
    /// A DB error occurred.
    #[error("a DB error occurred: {0}")]
    Rusqlite(#[from] rusqlite::Error),
    /// A decoding error occurred.
    #[error("failed to decode: {0}")]
    Decode(#[from] DecodeError),
}
