//! SQLite-backed persistence for metrics, health, and selector state.

/// Health data collectors backed by `SQLite`.
pub mod collectors;
/// Error types for `SQLite` operations.
pub mod error;
/// `SQLite`-based credential selector with weighted scoring.
pub mod selector;
/// SQLite store configuration and connection management.
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
