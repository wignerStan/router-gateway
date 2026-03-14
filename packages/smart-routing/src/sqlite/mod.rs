/// `SQLite` health and metrics collectors.
pub mod collectors;
/// `SQLite`-specific error types.
pub mod error;
/// `SQLite`-backed credential selector with weight queries.
pub mod selector;
/// `SQLite` store for persistent data.
pub mod store;

#[cfg(test)]
mod tests;

pub use collectors::SQLiteHealthManager;
pub use collectors::SQLiteMetricsCollector;
pub use error::{Result, SqliteError};
pub use selector::SQLiteSelector;
pub use selector::SelectorStats;
pub use store::SQLiteConfig;
pub use store::SQLiteStore;
