use crate::health::{AuthHealth, HealthStatus};
use crate::metrics::AuthMetrics;
use chrono::{DateTime, Utc};
use rusqlite::Connection;
use std::sync::{Arc, RwLock};
use tokio::sync::Mutex as AsyncMutex;

/// SQLite storage backend with WAL mode for concurrent read/write
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

/// Cache entry for hot data
#[derive(Clone)]
struct CacheEntry {
    metrics: Option<AuthMetrics>,
    health: Option<AuthHealth>,
    timestamp: DateTime<Utc>,
}

/// SQLite configuration
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
    /// Create a new SQLite store
    pub async fn new(config: SQLiteConfig) -> Result<Self, String> {
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
        let conn =
            Connection::open(&dsn).map_err(|e| format!("Failed to open SQLite database: {}", e))?;

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

    /// Configure SQLite pragmas
    async fn configure_pragmas(&self, config: &SQLiteConfig) -> Result<(), String> {
        let db = self.db.lock().await;

        // Configure cache size (negative value means KB)
        db.execute(
            &format!("PRAGMA cache_size = -{}", config.cache_size_mb * 1024),
            [],
        )
        .map_err(|e| format!("Failed to set cache_size: {}", e))?;

        // Configure busy timeout using the rusqlite method
        db.busy_timeout(std::time::Duration::from_millis(
            config.busy_timeout_ms as u64,
        ))
        .map_err(|e| format!("Failed to set busy_timeout: {}", e))?;

        // Enable foreign keys
        db.execute("PRAGMA foreign_keys = ON", [])
            .map_err(|e| format!("Failed to enable foreign_keys: {}", e))?;

        // Set synchronous mode to NORMAL for performance
        db.execute("PRAGMA synchronous = NORMAL", [])
            .map_err(|e| format!("Failed to set synchronous: {}", e))?;

        // Use memory for temp storage
        db.execute("PRAGMA temp_store = MEMORY", [])
            .map_err(|e| format!("Failed to set temp_store: {}", e))?;

        // Enable memory-mapped I/O
        db.execute("PRAGMA mmap_size = 268435456", []) // 256MB
            .map_err(|e| format!("Failed to set mmap_size: {}", e))?;

        Ok(())
    }

    /// Create database tables
    async fn create_tables(&self) -> Result<(), String> {
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
        .map_err(|e| format!("Failed to create auth_metrics table: {}", e))?;

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
        .map_err(|e| format!("Failed to create auth_health table: {}", e))?;

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
        .map_err(|e| format!("Failed to create status_code_history table: {}", e))?;

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
        .map_err(|e| format!("Failed to create auth_weights table: {}", e))?;

        Ok(())
    }

    /// Create database indexes
    async fn create_indexes(&self) -> Result<(), String> {
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
                .map_err(|e| format!("Failed to create index: {}", e))?;
        }

        Ok(())
    }

    /// Get database connection (for internal use)
    pub async fn get_db(&self) -> Arc<AsyncMutex<Connection>> {
        Arc::clone(&self.db)
    }

    /// Write metrics to database
    pub async fn write_metrics(&self, auth_id: &str, metrics: &AuthMetrics) -> Result<(), String> {
        let db = self.db.lock().await;

        let last_request_time = metrics.last_request_time.to_rfc3339();
        let last_success_time = metrics.last_success_time.map(|t| t.to_rfc3339());
        let last_failure_time = metrics.last_failure_time.map(|t| t.to_rfc3339());

        db.execute(
            "INSERT INTO auth_metrics (
                auth_id, total_requests, success_count, failure_count,
                avg_latency_ms, min_latency_ms, max_latency_ms,
                success_rate, error_rate,
                consecutive_successes, consecutive_failures,
                last_request_time, last_success_time, last_failure_time,
                updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, datetime('now'))
            ON CONFLICT(auth_id) DO UPDATE SET
                total_requests = excluded.total_requests,
                success_count = excluded.success_count,
                failure_count = excluded.failure_count,
                avg_latency_ms = excluded.avg_latency_ms,
                min_latency_ms = excluded.min_latency_ms,
                max_latency_ms = excluded.max_latency_ms,
                success_rate = excluded.success_rate,
                error_rate = excluded.error_rate,
                consecutive_successes = excluded.consecutive_successes,
                consecutive_failures = excluded.consecutive_failures,
                last_request_time = excluded.last_request_time,
                last_success_time = excluded.last_success_time,
                last_failure_time = excluded.last_failure_time,
                updated_at = datetime('now')",
            rusqlite::params![
                auth_id,
                metrics.total_requests,
                metrics.success_count,
                metrics.failure_count,
                metrics.avg_latency_ms,
                metrics.min_latency_ms,
                metrics.max_latency_ms,
                metrics.success_rate,
                metrics.error_rate,
                metrics.consecutive_successes,
                metrics.consecutive_failures,
                last_request_time,
                last_success_time,
                last_failure_time,
            ],
        )
        .map_err(|e| format!("Failed to write metrics: {}", e))?;

        // Update cache
        if self
            .cache_enabled
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            let mut cache = self.cache.write().unwrap();
            let entry = cache
                .entry(auth_id.to_string())
                .or_insert_with(|| CacheEntry {
                    metrics: None,
                    health: None,
                    timestamp: Utc::now(),
                });
            entry.metrics = Some(metrics.clone());
            entry.timestamp = Utc::now();
        }

        Ok(())
    }

    /// Write health to database
    pub async fn write_health(&self, auth_id: &str, health: &AuthHealth) -> Result<(), String> {
        let db = self.db.lock().await;

        let status = format!("{:?}", health.status);
        let last_status_change = health.last_status_change.to_rfc3339();
        let last_check_time = health.last_check_time.to_rfc3339();
        let unavailable_until = health.unavailable_until.map(|t| t.to_rfc3339());
        let error_counts =
            serde_json::to_string(&health.error_counts).unwrap_or_else(|_| "{}".to_string());

        db.execute(
            "INSERT INTO auth_health (
                auth_id, status, consecutive_successes, consecutive_failures,
                last_status_change, last_check_time, unavailable_until,
                error_counts, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, datetime('now'))
            ON CONFLICT(auth_id) DO UPDATE SET
                status = excluded.status,
                consecutive_successes = excluded.consecutive_successes,
                consecutive_failures = excluded.consecutive_failures,
                last_status_change = excluded.last_status_change,
                last_check_time = excluded.last_check_time,
                unavailable_until = excluded.unavailable_until,
                error_counts = excluded.error_counts,
                updated_at = datetime('now')",
            rusqlite::params![
                auth_id,
                status,
                health.consecutive_successes,
                health.consecutive_failures,
                last_status_change,
                last_check_time,
                unavailable_until,
                error_counts,
            ],
        )
        .map_err(|e| format!("Failed to write health: {}", e))?;

        // Update cache
        if self
            .cache_enabled
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            let mut cache = self.cache.write().unwrap();
            let entry = cache
                .entry(auth_id.to_string())
                .or_insert_with(|| CacheEntry {
                    metrics: None,
                    health: None,
                    timestamp: Utc::now(),
                });
            entry.health = Some(health.clone());
            entry.timestamp = Utc::now();
        }

        Ok(())
    }

    /// Write status code history entry
    pub async fn write_status_history(
        &self,
        auth_id: &str,
        status_code: i32,
        latency_ms: f64,
        success: bool,
    ) -> Result<(), String> {
        let db = self.db.lock().await;

        db.execute(
            "INSERT INTO status_code_history (auth_id, status_code, latency_ms, success, timestamp)
            VALUES (?1, ?2, ?3, ?4, datetime('now'))",
            rusqlite::params![
                auth_id,
                status_code,
                latency_ms,
                if success { 1 } else { 0 },
            ],
        )
        .map_err(|e| format!("Failed to write status history: {}", e))?;

        Ok(())
    }

    /// Load metrics from database
    pub async fn load_metrics(&self, auth_id: &str) -> Result<Option<AuthMetrics>, String> {
        // Check cache first
        if self
            .cache_enabled
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            let cache = self.cache.read().unwrap();
            if let Some(entry) = cache.get(auth_id) {
                if let Some(ref metrics) = entry.metrics {
                    return Ok(Some(metrics.clone()));
                }
            }
        }

        let db = self.db.lock().await;

        let mut stmt = db
            .prepare(
                "SELECT total_requests, success_count, failure_count,
                avg_latency_ms, min_latency_ms, max_latency_ms,
                success_rate, error_rate,
                consecutive_successes, consecutive_failures,
                last_request_time, last_success_time, last_failure_time
                FROM auth_metrics WHERE auth_id = ?1",
            )
            .map_err(|e| format!("Failed to prepare metrics query: {}", e))?;

        let result = stmt.query_row([auth_id], |row| {
            let last_request_time_str: String = row.get(10)?;
            let last_success_time_str: Option<String> = row.get(11)?;
            let last_failure_time_str: Option<String> = row.get(12)?;

            Ok(AuthMetrics {
                total_requests: row.get(0)?,
                success_count: row.get(1)?,
                failure_count: row.get(2)?,
                avg_latency_ms: row.get(3)?,
                min_latency_ms: row.get(4)?,
                max_latency_ms: row.get(5)?,
                success_rate: row.get(6)?,
                error_rate: row.get(7)?,
                consecutive_successes: row.get(8)?,
                consecutive_failures: row.get(9)?,
                last_request_time: DateTime::parse_from_rfc3339(&last_request_time_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            10,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        )
                    })?,
                last_success_time: last_success_time_str
                    .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&Utc)),
                last_failure_time: last_failure_time_str
                    .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&Utc)),
            })
        });

        match result {
            Ok(metrics) => {
                // Update cache
                if self
                    .cache_enabled
                    .load(std::sync::atomic::Ordering::Relaxed)
                {
                    let mut cache = self.cache.write().unwrap();
                    let entry = cache
                        .entry(auth_id.to_string())
                        .or_insert_with(|| CacheEntry {
                            metrics: None,
                            health: None,
                            timestamp: Utc::now(),
                        });
                    entry.metrics = Some(metrics.clone());
                    entry.timestamp = Utc::now();
                }
                Ok(Some(metrics))
            },
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(format!("Failed to load metrics: {}", e)),
        }
    }

    /// Load health from database
    pub async fn load_health(&self, auth_id: &str) -> Result<Option<AuthHealth>, String> {
        // Check cache first
        if self
            .cache_enabled
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            let cache = self.cache.read().unwrap();
            if let Some(entry) = cache.get(auth_id) {
                if let Some(ref health) = entry.health {
                    return Ok(Some(health.clone()));
                }
            }
        }

        let db = self.db.lock().await;

        let mut stmt = db
            .prepare(
                "SELECT status, consecutive_successes, consecutive_failures,
                last_status_change, last_check_time, unavailable_until, error_counts
                FROM auth_health WHERE auth_id = ?1",
            )
            .map_err(|e| format!("Failed to prepare health query: {}", e))?;

        let result = stmt.query_row([auth_id], |row| {
            let status_str: String = row.get(0)?;
            let last_status_change_str: String = row.get(3)?;
            let last_check_time_str: String = row.get(4)?;
            let unavailable_until_str: Option<String> = row.get(5)?;
            let error_counts_str: String = row.get(6)?;

            let status = match status_str.as_str() {
                "Degraded" => HealthStatus::Degraded,
                "Unhealthy" => HealthStatus::Unhealthy,
                _ => HealthStatus::Healthy,
            };

            let error_counts: std::collections::HashMap<i32, i32> =
                serde_json::from_str(&error_counts_str).unwrap_or_default();

            Ok(AuthHealth {
                status,
                consecutive_successes: row.get(1)?,
                consecutive_failures: row.get(2)?,
                last_status_change: DateTime::parse_from_rfc3339(&last_status_change_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            3,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        )
                    })?,
                last_check_time: DateTime::parse_from_rfc3339(&last_check_time_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            4,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        )
                    })?,
                unavailable_until: unavailable_until_str
                    .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&Utc)),
                error_counts,
            })
        });

        match result {
            Ok(health) => {
                // Update cache
                if self
                    .cache_enabled
                    .load(std::sync::atomic::Ordering::Relaxed)
                {
                    let mut cache = self.cache.write().unwrap();
                    let entry = cache
                        .entry(auth_id.to_string())
                        .or_insert_with(|| CacheEntry {
                            metrics: None,
                            health: None,
                            timestamp: Utc::now(),
                        });
                    entry.health = Some(health.clone());
                    entry.timestamp = Utc::now();
                }
                Ok(Some(health))
            },
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(format!("Failed to load health: {}", e)),
        }
    }

    /// Load all metrics from database
    pub async fn load_all_metrics(
        &self,
    ) -> Result<std::collections::HashMap<String, AuthMetrics>, String> {
        let db = self.db.lock().await;

        let mut stmt = db
            .prepare(
                "SELECT auth_id, total_requests, success_count, failure_count,
                avg_latency_ms, min_latency_ms, max_latency_ms,
                success_rate, error_rate,
                consecutive_successes, consecutive_failures,
                last_request_time, last_success_time, last_failure_time
                FROM auth_metrics",
            )
            .map_err(|e| format!("Failed to prepare all metrics query: {}", e))?;

        let metrics_map = stmt
            .query_map([], |row| {
                // Column indices:
                // 0: auth_id
                // 1: total_requests
                // 2: success_count
                // 3: failure_count
                // 4: avg_latency_ms
                // 5: min_latency_ms
                // 6: max_latency_ms
                // 7: success_rate
                // 8: error_rate
                // 9: consecutive_successes
                // 10: consecutive_failures
                // 11: last_request_time
                // 12: last_success_time
                // 13: last_failure_time
                let auth_id: String = row.get(0)?;
                let last_request_time_str: String = row.get(11)?;
                let last_success_time_str: Option<String> = row.get(12)?;
                let last_failure_time_str: Option<String> = row.get(13)?;

                Ok((
                    auth_id,
                    AuthMetrics {
                        total_requests: row.get(1)?,
                        success_count: row.get(2)?,
                        failure_count: row.get(3)?,
                        avg_latency_ms: row.get(4)?,
                        min_latency_ms: row.get(5)?,
                        max_latency_ms: row.get(6)?,
                        success_rate: row.get(7)?,
                        error_rate: row.get(8)?,
                        consecutive_successes: row.get(9)?,
                        consecutive_failures: row.get(10)?,
                        last_request_time: DateTime::parse_from_rfc3339(&last_request_time_str)
                            .map(|dt| dt.with_timezone(&Utc))
                            .unwrap_or_else(|_| Utc::now()),
                        last_success_time: last_success_time_str
                            .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                            .map(|dt| dt.with_timezone(&Utc)),
                        last_failure_time: last_failure_time_str
                            .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                            .map(|dt| dt.with_timezone(&Utc)),
                    },
                ))
            })
            .map_err(|e| format!("Failed to map metrics: {}", e))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("Failed to collect metrics: {}", e))?;

        Ok(metrics_map.into_iter().collect())
    }

    /// Load all health from database
    pub async fn load_all_health(
        &self,
    ) -> Result<std::collections::HashMap<String, AuthHealth>, String> {
        let db = self.db.lock().await;

        let mut stmt = db
            .prepare(
                "SELECT auth_id, status, consecutive_successes, consecutive_failures,
                last_status_change, last_check_time, unavailable_until, error_counts
                FROM auth_health",
            )
            .map_err(|e| format!("Failed to prepare all health query: {}", e))?;

        let health_map = stmt
            .query_map([], |row| {
                let auth_id: String = row.get(0)?;
                let status_str: String = row.get(1)?;
                let last_status_change_str: String = row.get(3)?;
                let last_check_time_str: String = row.get(4)?;
                let unavailable_until_str: Option<String> = row.get(5)?;
                let error_counts_str: String = row.get(6)?;

                let status = match status_str.as_str() {
                    "Degraded" => HealthStatus::Degraded,
                    "Unhealthy" => HealthStatus::Unhealthy,
                    _ => HealthStatus::Healthy,
                };

                let error_counts: std::collections::HashMap<i32, i32> =
                    serde_json::from_str(&error_counts_str).unwrap_or_default();

                Ok((
                    auth_id,
                    AuthHealth {
                        status,
                        consecutive_successes: row.get(2)?,
                        consecutive_failures: row.get(3)?,
                        last_status_change: DateTime::parse_from_rfc3339(&last_status_change_str)
                            .map(|dt| dt.with_timezone(&Utc))
                            .unwrap_or_else(|_| Utc::now()),
                        last_check_time: DateTime::parse_from_rfc3339(&last_check_time_str)
                            .map(|dt| dt.with_timezone(&Utc))
                            .unwrap_or_else(|_| Utc::now()),
                        unavailable_until: unavailable_until_str
                            .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                            .map(|dt| dt.with_timezone(&Utc)),
                        error_counts,
                    },
                ))
            })
            .map_err(|e| format!("Failed to map health: {}", e))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("Failed to collect health: {}", e))?;

        Ok(health_map.into_iter().collect())
    }

    /// Cleanup old history records
    pub async fn cleanup_old_history(&self, max_age_seconds: i64) -> Result<i64, String> {
        let cutoff = Utc::now() - chrono::Duration::seconds(max_age_seconds);
        let cutoff_str = cutoff.to_rfc3339();

        let db = self.db.lock().await;

        let result = db
            .execute(
                "DELETE FROM status_code_history WHERE timestamp < ?1",
                [&cutoff_str],
            )
            .map_err(|e| format!("Failed to cleanup old history: {}", e))?;

        Ok(result as i64)
    }

    /// Get history statistics
    pub async fn get_history_stats(&self) -> Result<(i64, Option<DateTime<Utc>>), String> {
        let db = self.db.lock().await;

        let mut stmt = db
            .prepare("SELECT COUNT(*), MIN(timestamp) FROM status_code_history")
            .map_err(|e| format!("Failed to prepare history stats query: {}", e))?;

        stmt.query_row([], |row| {
            let count: i64 = row.get(0)?;
            let min_timestamp_str: Option<String> = row.get(1)?;

            let min_timestamp = min_timestamp_str
                .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc));

            Ok((count, min_timestamp))
        })
        .map_err(|e| format!("Failed to get history stats: {}", e))
    }
}
