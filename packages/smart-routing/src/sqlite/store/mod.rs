//! `SQLite` storage backend with WAL mode for concurrent read/write.

mod operations;

use super::error::{Result, SqliteError};
use crate::health::AuthHealth;
use crate::metrics::AuthMetrics;
use chrono::{DateTime, Utc};
use rusqlite::Connection;
use std::sync::{Arc, RwLock};
use tokio::sync::Mutex as AsyncMutex;

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
    /// Database connection (wrapped in Arc for thread safety)
    db: Arc<AsyncMutex<Connection>>,
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
    pub async fn new(config: SQLiteConfig) -> Result<Self> {
        let cfg = config.clone();

        // Build DSN with WAL mode if enabled
        let dsn = if cfg.enable_wal && cfg.database_path != ":memory:" {
            format!(
                "{}?mode=rwc&_journal=WAL&_busy_timeout={}",
                cfg.database_path, cfg.busy_timeout_ms
            )
        } else {
            cfg.database_path.clone()
        };

        // Open database connection
        let conn = Connection::open(&dsn)?;

        // Configure pragmas
        let store = Self {
            db: Arc::new(AsyncMutex::new(conn)),
            db_path: cfg.database_path.clone(),
            cache_enabled: Arc::new(std::sync::atomic::AtomicBool::new(cfg.enable_cache)),
            cache: Arc::new(RwLock::new(std::collections::HashMap::new())),
        };

        store.configure_pragmas(&cfg).await?;
        store.create_tables().await?;
        store.create_indexes().await?;

        Ok(store)
    }

    /// Configure `SQLite` pragmas
    async fn configure_pragmas(&self, config: &SQLiteConfig) -> Result<()> {
        let db = self.db.lock().await;

        // Configure cache size (negative value means KB)
        db.execute(
            &format!("PRAGMA cache_size = -{}", config.cache_size_mb * 1024),
            [],
        )
        .map_err(|e| SqliteError::query("set_cache_size", e))?;

        // Configure busy timeout using the rusqlite method
        db.busy_timeout(std::time::Duration::from_millis(
            config.busy_timeout_ms as u64,
        ))
        .map_err(|e| SqliteError::query("set_busy_timeout", e))?;

        // Enable foreign keys
        db.execute("PRAGMA foreign_keys = ON", [])
            .map_err(|e| SqliteError::query("set_foreign_keys", e))?;

        // Set synchronous mode to NORMAL for performance
        db.execute("PRAGMA synchronous = NORMAL", [])
            .map_err(|e| SqliteError::query("set_synchronous", e))?;

        // Use memory for temp storage
        db.execute("PRAGMA temp_store = MEMORY", [])
            .map_err(|e| SqliteError::query("set_temp_store", e))?;

        // Enable memory-mapped I/O
        db.execute("PRAGMA mmap_size = 268435456", []) // 256MB
            .map_err(|e| SqliteError::query("set_mmap_size", e))?;

        Ok(())
    }

    /// Create database tables
    async fn create_tables(&self) -> Result<()> {
        let db = self.db.lock().await;

        // Auth metrics table
        db.execute(
            "CREATE TABLE IF NOT EXISTS auth_metrics (
                auth_id TEXT PRIMARY KEY,
                total_requests INTEGER DEFAULT 0,
                success_count INTEGER DEFAULT 0,
                failure_count INTEGER DEFAULT 0,
                avg_latency_ms REAL DEFAULT 0,
                min_latency_ms REAL DEFAULT 0,
                max_latency_ms REAL DEFAULT 0,
                success_rate REAL DEFAULT 1.0,
                error_rate REAL DEFAULT 0,
                consecutive_successes INTEGER DEFAULT 0,
                consecutive_failures INTEGER DEFAULT 0,
                last_request_time TEXT,
                last_success_time TEXT,
                last_failure_time TEXT,
                created_at TEXT DEFAULT (datetime('now')),
                updated_at TEXT DEFAULT (datetime('now'))
            )",
            [],
        )
        .map_err(|e| SqliteError::Schema { source: e })?;

        // Auth health table
        db.execute(
            "CREATE TABLE IF NOT EXISTS auth_health (
                auth_id TEXT PRIMARY KEY,
                status TEXT DEFAULT 'Healthy',
                consecutive_successes INTEGER DEFAULT 0,
                consecutive_failures INTEGER DEFAULT 0,
                last_status_change TEXT,
                last_check_time TEXT,
                unavailable_until TEXT,
                error_counts TEXT DEFAULT '{}',
                created_at TEXT DEFAULT (datetime('now')),
                updated_at TEXT DEFAULT (datetime('now'))
            )",
            [],
        )
        .map_err(|e| SqliteError::Schema { source: e })?;

        // Status code history table
        db.execute(
            "CREATE TABLE IF NOT EXISTS status_code_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                auth_id TEXT NOT NULL,
                status_code INTEGER NOT NULL,
                latency_ms REAL,
                success INTEGER,
                timestamp TEXT DEFAULT (datetime('now'))
            )",
            [],
        )
        .map_err(|e| SqliteError::Schema { source: e })?;

        // Auth weights table
        db.execute(
            "CREATE TABLE IF NOT EXISTS auth_weights (
                auth_id TEXT PRIMARY KEY,
                weight REAL DEFAULT 1.0,
                calculated_at TEXT,
                strategy TEXT DEFAULT 'weighted'
            )",
            [],
        )
        .map_err(|e| SqliteError::Schema { source: e })?;

        Ok(())
    }

    /// Create database indexes
    async fn create_indexes(&self) -> Result<()> {
        let db = self.db.lock().await;

        let indexes = vec![
            "CREATE INDEX IF NOT EXISTS idx_weights_weight ON auth_weights(weight DESC)",
            "CREATE INDEX IF NOT EXISTS idx_weights_strategy ON auth_weights(strategy, weight DESC)",
            "CREATE INDEX IF NOT EXISTS idx_health_status ON auth_health(status)",
            "CREATE INDEX IF NOT EXISTS idx_health_unavailable ON auth_health(unavailable_until)",
            "CREATE INDEX IF NOT EXISTS idx_metrics_success_rate ON auth_metrics(success_rate)",
            "CREATE INDEX IF NOT EXISTS idx_metrics_latency ON auth_metrics(avg_latency_ms)",
            "CREATE INDEX IF NOT EXISTS idx_metrics_updated ON auth_metrics(updated_at)",
            "CREATE INDEX IF NOT EXISTS idx_status_auth_time ON status_code_history(auth_id, timestamp DESC)",
            "CREATE INDEX IF NOT EXISTS idx_status_code ON status_code_history(status_code)",
        ];

        for idx in indexes {
            db.execute(idx, [])
                .map_err(|e| SqliteError::Schema { source: e })?;
        }

        Ok(())
    }

    /// Get database connection (for internal use)
    pub fn get_db(&self) -> Arc<AsyncMutex<Connection>> {
        Arc::clone(&self.db)
    }
}
