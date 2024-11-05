use thiserror::Error;

/// A database or decoding error returned by a query.
#[derive(Debug, Error)]
pub enum QueryError {
    /// A DB error occurred.
    #[error("a DB error occurred: {0}")]
    Rusqlite(#[from] rusqlite::Error),
    /// Unsupported range used in query range.
    #[error("query range called with an unsupported range")]
    UnsupportedRange,
}
