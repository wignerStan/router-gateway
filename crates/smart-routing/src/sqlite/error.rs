use thiserror::Error;

/// SQLite-specific errors for the smart-routing persistence layer.
#[derive(Debug, Error)]
pub enum SqliteError {
    /// Database open or connection failure.
    #[error("cannot open database: {0}")]
    Connection(#[from] rusqlite::Error),
    /// Prepared statement execution, row read, or generic query failure.
    #[error("cannot execute {operation}: {source}")]
    Query {
        /// Name of the operation that failed
        operation: &'static str,
        /// Underlying rusqlite error
        source: rusqlite::Error,
    },
    /// Schema migration: CREATE TABLE, CREATE INDEX, PRAGMA.
    #[error("cannot apply schema migration: {source}")]
    Schema {
        /// Underlying rusqlite error
        source: rusqlite::Error,
    },
    /// Serde serialization/deserialization failure.
    #[error("cannot serialize data: {0}")]
    Serialization(String),
}

impl SqliteError {
    /// Convenience constructor for query errors.
    #[must_use]
    pub const fn query(operation: &'static str, source: rusqlite::Error) -> Self {
        Self::Query { operation, source }
    }
}

/// Result type for `SQLite` operations.
pub type Result<T> = std::result::Result<T, SqliteError>;
