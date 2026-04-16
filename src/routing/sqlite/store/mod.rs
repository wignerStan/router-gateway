//! `SQLite` storage backend with WAL mode for concurrent read/write.

mod operations;

use super::error::Result;
use crate::routing::health::AuthHealth;
use crate::routing::metrics::AuthMetrics;
use chrono::{DateTime, Utc};
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use std::str::FromStr;
use std::sync::{Arc, RwLock};
use std::time::Duration;

/// Cache entry for hot data
#[derive(Clone)]
struct CacheEntry {
    metrics: Option<AuthMetrics>,
    health: Option<AuthHealth>,
    timestamp: DateTime<Utc>,
}

/// `SQLite` storage backend with WAL mode for concurrent read/write
#[derive(Clone)]
pub struct SQLiteStore {
    /// Connection pool (async-native, no mutex needed)
    pool: SqlitePool,
    /// Database path
    #[allow(dead_code)]
    db_path: String,
    /// Cache enabled flag
    cache_enabled: Arc<std::sync::atomic::AtomicBool>,
    /// Hot data cache
    cache: Arc<RwLock<std::collections::HashMap<String, CacheEntry>>>,
}

/// `SQLite` configuration
#[derive(Clone, Debug)]
pub struct SQLiteConfig {
    /// Database file path
    pub database_path: String,
    /// Enable WAL mode
    pub enable_wal: bool,
    /// Cache size in MB
    pub cache_size_mb: i64,
    /// Busy timeout in milliseconds
    pub busy_timeout_ms: i64,
    /// Batch size for writes
    pub batch_size: usize,
    /// Enable hot data cache
    pub enable_cache: bool,
}

impl Default for SQLiteConfig {
    fn default() -> Self {
        Self {
            database_path: ":memory:".to_string(),
            enable_wal: true,
            cache_size_mb: 10,
            busy_timeout_ms: 5000,
            batch_size: 100,
            enable_cache: true,
        }
    }
}

impl SQLiteStore {
    /// Create a new `SQLite` store
    ///
    /// # Errors
    ///
    /// Returns [`SqliteError::Connection`] if the database cannot be opened.
    /// Returns [`SqliteError::Schema`] if migration fails.
    pub async fn new(config: SQLiteConfig) -> Result<Self> {
        let cfg = config.clone();

        let options = if cfg.database_path == ":memory:" {
            SqliteConnectOptions::from_str("sqlite::memory:")?
                .journal_mode(SqliteJournalMode::Wal)
                .busy_timeout(Duration::from_millis(cfg.busy_timeout_ms as u64))
                .foreign_keys(true)
                .synchronous(SqliteSynchronous::Normal)
        } else {
            SqliteConnectOptions::from_str(&format!("sqlite:{}", cfg.database_path))?
                .create_if_missing(true)
                .journal_mode(if cfg.enable_wal {
                    SqliteJournalMode::Wal
                } else {
                    SqliteJournalMode::Delete
                })
                .busy_timeout(Duration::from_millis(cfg.busy_timeout_ms as u64))
                .foreign_keys(true)
                .synchronous(SqliteSynchronous::Normal)
        };

        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(options)
            .await?;

        // Run migrations
        sqlx::migrate!("./migrations").run(&pool).await?;

        let store = Self {
            pool,
            db_path: cfg.database_path.clone(),
            cache_enabled: Arc::new(std::sync::atomic::AtomicBool::new(cfg.enable_cache)),
            cache: Arc::new(RwLock::new(std::collections::HashMap::new())),
        };

        Ok(store)
    }

    /// Create store from an existing pool (for testing).
    ///
    /// Assumes migrations have already been applied to the pool.
    #[must_use]
    pub fn from_pool(pool: SqlitePool) -> Self {
        Self {
            pool,
            db_path: ":memory:".to_string(),
            cache_enabled: Arc::new(std::sync::atomic::AtomicBool::new(true)),
            cache: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    /// Get database pool (for internal use by selector)
    #[must_use]
    pub fn get_pool(&self) -> SqlitePool {
        self.pool.clone()
    }
}
