use thiserror::Error;

/// Any error that might occur during decoding of a type returned by the DB.
#[derive(Debug, Error)]
pub enum DecodeError {
    /// Failed to decode the hex string to bytes.
    #[error("hex decoding failed: {0}")]
    Hex(#[from] hex::FromHexError),
    /// Failed to deserialize from the decoded bytes.
    #[error("deserialization failed: {0}")]
    Postcard(#[from] postcard::Error),
}

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
