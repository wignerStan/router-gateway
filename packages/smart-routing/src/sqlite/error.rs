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
        operation: &'static str,
        source: rusqlite::Error,
    },
    /// Schema migration: CREATE TABLE, CREATE INDEX, PRAGMA.
    #[error("cannot apply schema migration: {source}")]
    Schema { source: rusqlite::Error },
    /// Serde serialization/deserialization failure.
    #[error("cannot serialize data: {0}")]
    Serialization(String),
}

impl SqliteError {
    /// Convenience constructor for query errors.
    pub const fn query(operation: &'static str, source: rusqlite::Error) -> Self {
        Self::Query { operation, source }
    }
}

pub type Result<T> = std::result::Result<T, SqliteError>;
