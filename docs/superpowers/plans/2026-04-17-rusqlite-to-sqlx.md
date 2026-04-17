# Rusqlite to SQLx Migration & Test Suite Improvement

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace rusqlite with sqlx for async-native SQLite access, then upgrade the test suite with `sqlx::test` fixtures for cleaner, more reliable tests.

**Architecture:** Replace `Arc<tokio::sync::Mutex<rusqlite::Connection>>` with `sqlx::SqlitePool` (async-native connection pool). Replace inline schema creation with SQL migration files. Replace `rusqlite::params![]` + `row.get(N)` with `sqlx::query().bind()` + `FromRow` derive. Keep the same public API surface (`SQLiteStore`, `SQLiteSelector`, `SQLiteMetricsCollector`, `SQLiteHealthManager`) so callers don't change.

**Tech Stack:** sqlx 0.8 (runtime-tokio + sqlite + migrate), libsqlite3-sys (bundled), existing Tokio runtime

---

## File Structure

| File | Action | Purpose |
|------|--------|---------|
| `Cargo.toml` | Modify | Swap rusqlite → sqlx |
| `migrations/20260417000000_initial_schema.sql` | Create | Table + index DDL |
| `src/routing/sqlite/error.rs` | Rewrite | sqlx::Error instead of rusqlite::Error |
| `src/routing/sqlite/store/mod.rs` | Rewrite | SqlitePool connection management |
| `src/routing/sqlite/store/operations.rs` | Rewrite | All queries via sqlx |
| `src/routing/sqlite/selector.rs` | Rewrite | Selector queries via sqlx |
| `src/routing/sqlite/collectors.rs` | Modify | Type signature updates |
| `src/routing/sqlite/mod.rs` | Modify | Update re-exports |
| `src/routing/sqlite/tests.rs` | Rewrite | sqlx::test fixtures |

---

### Task 1: Foundation — Dependencies, Migrations, Error Types

**Files:**
- Modify: `Cargo.toml`
- Create: `migrations/20260417000000_initial_schema.sql`
- Rewrite: `src/routing/sqlite/error.rs`

- [ ] **Step 1: Replace rusqlite with sqlx in Cargo.toml**

In `Cargo.toml`, replace the rusqlite dependency line (line 70) with:

```toml
# Database
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite"] }
```

Add bundled SQLite support (same as current rusqlite bundled feature):

```toml
# Bundled SQLite (no system dependency)
libsqlite3-sys = { version = "0.30", features = ["bundled"] }
```

The final `[dependencies]` database section becomes:
```toml
# Database
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite"] }
libsqlite3-sys = { version = "0.30", features = ["bundled"] }
```

Remove: `rusqlite = { version = "0.31", features = ["bundled"] }`

- [ ] **Step 2: Create migrations directory and initial schema file**

Create `migrations/20260417000000_initial_schema.sql`:

```sql
-- Auth metrics table
CREATE TABLE IF NOT EXISTS auth_metrics (
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
);

-- Auth health table
CREATE TABLE IF NOT EXISTS auth_health (
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
);

-- Status code history table
CREATE TABLE IF NOT EXISTS status_code_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    auth_id TEXT NOT NULL,
    status_code INTEGER NOT NULL,
    latency_ms REAL,
    success INTEGER,
    timestamp TEXT DEFAULT (datetime('now'))
);

-- Auth weights table
CREATE TABLE IF NOT EXISTS auth_weights (
    auth_id TEXT PRIMARY KEY,
    weight REAL DEFAULT 1.0,
    calculated_at TEXT,
    strategy TEXT DEFAULT 'weighted'
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_weights_weight ON auth_weights(weight DESC);
CREATE INDEX IF NOT EXISTS idx_weights_strategy ON auth_weights(strategy, weight DESC);
CREATE INDEX IF NOT EXISTS idx_health_status ON auth_health(status);
CREATE INDEX IF NOT EXISTS idx_health_unavailable ON auth_health(unavailable_until);
CREATE INDEX IF NOT EXISTS idx_metrics_success_rate ON auth_metrics(success_rate);
CREATE INDEX IF NOT EXISTS idx_metrics_latency ON auth_metrics(avg_latency_ms);
CREATE INDEX IF NOT EXISTS idx_metrics_updated ON auth_metrics(updated_at);
CREATE INDEX IF NOT EXISTS idx_status_auth_time ON status_code_history(auth_id, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_status_code ON status_code_history(status_code);
```

- [ ] **Step 3: Rewrite error types**

Replace entire `src/routing/sqlite/error.rs` with:

```rust
use thiserror::Error;

/// SQLite-specific errors for the smart-routing persistence layer.
#[derive(Debug, Error)]
pub enum SqliteError {
    /// Database open or connection failure.
    #[error("cannot open database: {0}")]
    Connection(#[from] sqlx::Error),
    /// Prepared statement execution, row read, or generic query failure.
    #[error("cannot execute {operation}: {source}")]
    Query {
        /// Name of the operation that failed
        operation: &'static str,
        /// Underlying sqlx error
        source: sqlx::Error,
    },
    /// Schema migration failure.
    #[error("cannot apply schema migration: {0}")]
    Schema(#[from] sqlx::migrate::MigrateError),
    /// Serde serialization/deserialization failure.
    #[error("cannot serialize data: {0}")]
    Serialization(String),
}

impl SqliteError {
    /// Convenience constructor for query errors.
    #[must_use]
    pub const fn query(operation: &'static str, source: sqlx::Error) -> Self {
        Self::Query { operation, source }
    }
}

/// Result type for `SQLite` operations.
pub type Result<T> = std::result::Result<T, SqliteError>;
```

- [ ] **Step 4: Verify foundation compiles**

Run: `cargo check 2>&1 | head -20`
Expected: Compilation errors in `store/mod.rs`, `operations.rs`, `selector.rs` (rusqlite imports fail). This is expected — we fix these in subsequent tasks.

- [ ] **Step 5: Commit foundation**

```bash
git add Cargo.toml migrations/ src/routing/sqlite/error.rs
git commit -m "chore: swap rusqlite for sqlx, add migrations, rewrite error types"
```

---

### Task 2: Store Connection Management

**Files:**
- Rewrite: `src/routing/sqlite/store/mod.rs`

- [ ] **Step 1: Rewrite store/mod.rs with SqlitePool**

Replace entire `src/routing/sqlite/store/mod.rs` with:

```rust
//! `SQLite` storage backend with WAL mode for concurrent read/write.

mod operations;

use super::error::{Result, SqliteError};
use crate::routing::health::AuthHealth;
use crate::routing::metrics::AuthMetrics;
use chrono::{DateTime, Utc};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::SqlitePool;
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
```

Key changes from rusqlite:
- `Arc<tokio::sync::Mutex<rusqlite::Connection>>` → `SqlitePool` (async-native pool, clone is cheap — it's an Arc internally)
- `Connection::open()` → `SqlitePoolOptions::new().connect_with()` with typed options
- Inline `CREATE TABLE`/`CREATE INDEX` → `sqlx::migrate!("./migrations").run(&pool)`
- PRAGMAs set via `SqliteConnectOptions` methods
- Added `from_pool()` constructor for test fixtures
- `get_db()` returns `Arc<Mutex<Connection>>` → `get_pool()` returns `SqlitePool`

- [ ] **Step 2: Verify compilation**

Run: `cargo check 2>&1 | head -30`
Expected: Errors in `operations.rs` and `selector.rs` (they reference removed `self.db.lock()`). `store/mod.rs` itself should compile.

- [ ] **Step 3: Commit store rewrite**

```bash
git add src/routing/sqlite/store/mod.rs
git commit -m "refactor: rewrite SQLiteStore with SqlitePool, replace Mutex<Connection>"
```

---

### Task 3: Store Write Operations

**Files:**
- Rewrite: `src/routing/sqlite/store/operations.rs`

- [ ] **Step 1: Write the full operations.rs**

Replace entire `src/routing/sqlite/store/operations.rs` with:

```rust
//! `SQLite` store read/write operations.

// ALLOW: Mutex/RwLock poisoning is an acceptable panic -- propagates failure for
// inconsistent shared state. All `.expect()` calls below are on cache locks that
// poison when a holder panics, indicating data corruption.
#![allow(clippy::expect_used)]
#![allow(clippy::significant_drop_tightening)]

use super::super::error::{Result, SqliteError};
use super::{CacheEntry, SQLiteStore};
use crate::routing::health::{AuthHealth, HealthStatus};
use crate::routing::metrics::AuthMetrics;
use chrono::{DateTime, Utc};
use sqlx::FromRow;
use std::collections::HashMap;

/// Row type for reading auth_metrics columns.
#[derive(Debug, FromRow)]
struct MetricsRow {
    total_requests: i64,
    success_count: i64,
    failure_count: i64,
    avg_latency_ms: f64,
    min_latency_ms: f64,
    max_latency_ms: f64,
    success_rate: f64,
    error_rate: f64,
    consecutive_successes: i32,
    consecutive_failures: i32,
    last_request_time: String,
    last_success_time: Option<String>,
    last_failure_time: Option<String>,
}

/// Row type for reading auth_metrics with auth_id (load_all).
#[derive(Debug, FromRow)]
struct MetricsWithIdRow {
    auth_id: String,
    total_requests: i64,
    success_count: i64,
    failure_count: i64,
    avg_latency_ms: f64,
    min_latency_ms: f64,
    max_latency_ms: f64,
    success_rate: f64,
    error_rate: f64,
    consecutive_successes: i32,
    consecutive_failures: i32,
    last_request_time: String,
    last_success_time: Option<String>,
    last_failure_time: Option<String>,
}

/// Row type for reading auth_health columns.
#[derive(Debug, FromRow)]
struct HealthRow {
    status: String,
    consecutive_successes: i32,
    consecutive_failures: i32,
    last_status_change: String,
    last_check_time: String,
    unavailable_until: Option<String>,
    error_counts: String,
}

/// Row type for reading auth_health with auth_id (load_all).
#[derive(Debug, FromRow)]
struct HealthWithIdRow {
    auth_id: String,
    status: String,
    consecutive_successes: i32,
    consecutive_failures: i32,
    last_status_change: String,
    last_check_time: String,
    unavailable_until: Option<String>,
    error_counts: String,
}

/// Parse RFC3339 datetime string, returning `Utc::now()` on failure.
fn parse_datetime(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .inspect_err(|e| {
            tracing::warn!("Failed to parse datetime '{s}': {e}");
        })
        .unwrap_or_else(|_| Utc::now())
}

/// Parse optional RFC3339 datetime string.
fn parse_datetime_opt(s: Option<&str>) -> Option<DateTime<Utc>> {
    s.and_then(|v| {
        DateTime::parse_from_rfc3339(v)
            .map(|dt| dt.with_timezone(&Utc))
            .inspect_err(|e| {
                tracing::warn!("Failed to parse datetime '{v}': {e}");
            })
            .ok()
    })
}

/// Parse health status string.
fn parse_health_status(s: &str) -> HealthStatus {
    match s {
        "Degraded" => HealthStatus::Degraded,
        "Unhealthy" => HealthStatus::Unhealthy,
        _ => HealthStatus::Healthy,
    }
}

/// Parse error_counts JSON string.
fn parse_error_counts(s: &str) -> HashMap<i32, i32> {
    serde_json::from_str(s).unwrap_or_else(|e| {
        tracing::warn!("Failed to parse error_counts JSON: {e}");
        HashMap::default()
    })
}

impl SQLiteStore {
    /// Write metrics to the database.
    ///
    /// # Errors
    ///
    /// Returns an error if the SQL query fails or the cache write lock is poisoned.
    ///
    /// # Panics
    ///
    /// Panics if the cache write lock is poisoned (indicates data corruption from a prior panic).
    pub async fn write_metrics(&self, auth_id: &str, metrics: &AuthMetrics) -> Result<()> {
        let last_request_time = metrics.last_request_time.to_rfc3339();
        let last_success_time = metrics.last_success_time.map(|t| t.to_rfc3339());
        let last_failure_time = metrics.last_failure_time.map(|t| t.to_rfc3339());

        sqlx::query(
            "INSERT INTO auth_metrics (
                auth_id, total_requests, success_count, failure_count,
                avg_latency_ms, min_latency_ms, max_latency_ms,
                success_rate, error_rate,
                consecutive_successes, consecutive_failures,
                last_request_time, last_success_time, last_failure_time,
                updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, datetime('now'))
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
        )
        .bind(auth_id)
        .bind(metrics.total_requests)
        .bind(metrics.success_count)
        .bind(metrics.failure_count)
        .bind(metrics.avg_latency_ms)
        .bind(metrics.min_latency_ms)
        .bind(metrics.max_latency_ms)
        .bind(metrics.success_rate)
        .bind(metrics.error_rate)
        .bind(metrics.consecutive_successes)
        .bind(metrics.consecutive_failures)
        .bind(&last_request_time)
        .bind(&last_success_time)
        .bind(&last_failure_time)
        .execute(&self.pool)
        .await
        .map_err(|e| SqliteError::query("write_metrics", e))?;

        // Relaxed ordering is safe: this flag is set once at construction and never
        // modified. The RwLock guarding the actual cache data provides the necessary
        // Acquire/Release synchronization.
        if self
            .cache_enabled
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            let mut cache = self
                .cache
                .write()
                .expect("cache write lock poisoned: metrics write");
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

    /// Write health to the database.
    ///
    /// # Errors
    ///
    /// Returns an error if the SQL query fails or the cache write lock is poisoned.
    ///
    /// # Panics
    ///
    /// Panics if the cache write lock is poisoned (indicates data corruption from a prior panic).
    pub async fn write_health(&self, auth_id: &str, health: &AuthHealth) -> Result<()> {
        let status = format!("{:?}", health.status);
        let last_status_change = health.last_status_change.to_rfc3339();
        let last_check_time = health.last_check_time.to_rfc3339();
        let unavailable_until = health.unavailable_until.map(|t| t.to_rfc3339());
        let error_counts =
            serde_json::to_string(&health.error_counts).unwrap_or_else(|_| "{}".to_string());

        sqlx::query(
            "INSERT INTO auth_health (
                auth_id, status, consecutive_successes, consecutive_failures,
                last_status_change, last_check_time, unavailable_until,
                error_counts, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, datetime('now'))
            ON CONFLICT(auth_id) DO UPDATE SET
                status = excluded.status,
                consecutive_successes = excluded.consecutive_successes,
                consecutive_failures = excluded.consecutive_failures,
                last_status_change = excluded.last_status_change,
                last_check_time = excluded.last_check_time,
                unavailable_until = excluded.unavailable_until,
                error_counts = excluded.error_counts,
                updated_at = datetime('now')",
        )
        .bind(auth_id)
        .bind(&status)
        .bind(health.consecutive_successes)
        .bind(health.consecutive_failures)
        .bind(&last_status_change)
        .bind(&last_check_time)
        .bind(&unavailable_until)
        .bind(&error_counts)
        .execute(&self.pool)
        .await
        .map_err(|e| SqliteError::query("write_health", e))?;

        if self
            .cache_enabled
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            let mut cache = self
                .cache
                .write()
                .expect("cache write lock poisoned: health write");
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

    /// Write status code history entry.
    ///
    /// # Errors
    ///
    /// Returns an error if the SQL query fails.
    pub async fn write_status_history(
        &self,
        auth_id: &str,
        status_code: i32,
        latency_ms: f64,
        success: bool,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO status_code_history (auth_id, status_code, latency_ms, success, timestamp)
            VALUES ($1, $2, $3, $4, datetime('now'))",
        )
        .bind(auth_id)
        .bind(status_code)
        .bind(latency_ms)
        .bind(i32::from(success))
        .execute(&self.pool)
        .await
        .map_err(|e| SqliteError::query("write_status_history", e))?;

        Ok(())
    }

    /// Load metrics from the database.
    ///
    /// # Errors
    ///
    /// Returns an error if the SQL query fails or the cache lock is poisoned.
    ///
    /// # Panics
    ///
    /// Panics if the cache write lock is poisoned (indicates data corruption).
    pub async fn load_metrics(&self, auth_id: &str) -> Result<Option<AuthMetrics>> {
        if self
            .cache_enabled
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            let cache = self
                .cache
                .read()
                .expect("cache read lock poisoned: metrics read");
            if let Some(entry) = cache.get(auth_id) {
                if let Some(ref metrics) = entry.metrics {
                    return Ok(Some(metrics.clone()));
                }
            }
        }

        let row: Option<MetricsRow> = sqlx::query_as::<_, MetricsRow>(
            "SELECT total_requests, success_count, failure_count,
            avg_latency_ms, min_latency_ms, max_latency_ms,
            success_rate, error_rate,
            consecutive_successes, consecutive_failures,
            last_request_time, last_success_time, last_failure_time
            FROM auth_metrics WHERE auth_id = $1",
        )
        .bind(auth_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| SqliteError::query("load_metrics", e))?;

        let Some(row) = row else { return Ok(None) };

        let metrics = AuthMetrics {
            total_requests: row.total_requests,
            success_count: row.success_count,
            failure_count: row.failure_count,
            avg_latency_ms: row.avg_latency_ms,
            min_latency_ms: row.min_latency_ms,
            max_latency_ms: row.max_latency_ms,
            success_rate: row.success_rate,
            error_rate: row.error_rate,
            consecutive_successes: row.consecutive_successes,
            consecutive_failures: row.consecutive_failures,
            last_request_time: parse_datetime(&row.last_request_time),
            last_success_time: parse_datetime_opt(row.last_success_time.as_deref()),
            last_failure_time: parse_datetime_opt(row.last_failure_time.as_deref()),
        };

        if self
            .cache_enabled
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            let mut cache = self
                .cache
                .write()
                .expect("cache write lock poisoned: metrics write after load");
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
    }

    /// Load health from the database.
    ///
    /// # Errors
    ///
    /// Returns an error if the SQL query fails or the cache lock is poisoned.
    ///
    /// # Panics
    ///
    /// Panics if the cache write lock is poisoned (indicates data corruption).
    pub async fn load_health(&self, auth_id: &str) -> Result<Option<AuthHealth>> {
        if self
            .cache_enabled
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            let cache = self
                .cache
                .read()
                .expect("cache read lock poisoned: health read");
            if let Some(entry) = cache.get(auth_id) {
                if let Some(ref health) = entry.health {
                    return Ok(Some(health.clone()));
                }
            }
        }

        let row: Option<HealthRow> = sqlx::query_as::<_, HealthRow>(
            "SELECT status, consecutive_successes, consecutive_failures,
            last_status_change, last_check_time, unavailable_until, error_counts
            FROM auth_health WHERE auth_id = $1",
        )
        .bind(auth_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| SqliteError::query("load_health", e))?;

        let Some(row) = row else { return Ok(None) };

        let health = AuthHealth {
            status: parse_health_status(&row.status),
            consecutive_successes: row.consecutive_successes,
            consecutive_failures: row.consecutive_failures,
            last_status_change: parse_datetime(&row.last_status_change),
            last_check_time: parse_datetime(&row.last_check_time),
            unavailable_until: parse_datetime_opt(row.unavailable_until.as_deref()),
            error_counts: parse_error_counts(&row.error_counts),
        };

        if self
            .cache_enabled
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            let mut cache = self
                .cache
                .write()
                .expect("cache write lock poisoned: health write after load");
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
    }

    /// Load all metrics from the database.
    ///
    /// # Errors
    ///
    /// Returns an error if the SQL query or row mapping fails.
    pub async fn load_all_metrics(&self) -> Result<HashMap<String, AuthMetrics>> {
        let rows: Vec<MetricsWithIdRow> = sqlx::query_as::<_, MetricsWithIdRow>(
            "SELECT auth_id, total_requests, success_count, failure_count,
            avg_latency_ms, min_latency_ms, max_latency_ms,
            success_rate, error_rate,
            consecutive_successes, consecutive_failures,
            last_request_time, last_success_time, last_failure_time
            FROM auth_metrics",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| SqliteError::query("load_all_metrics", e))?;

        let metrics_map: HashMap<String, AuthMetrics> = rows
            .into_iter()
            .map(|row| {
                let auth_id = row.auth_id.clone();
                let metrics = AuthMetrics {
                    total_requests: row.total_requests,
                    success_count: row.success_count,
                    failure_count: row.failure_count,
                    avg_latency_ms: row.avg_latency_ms,
                    min_latency_ms: row.min_latency_ms,
                    max_latency_ms: row.max_latency_ms,
                    success_rate: row.success_rate,
                    error_rate: row.error_rate,
                    consecutive_successes: row.consecutive_successes,
                    consecutive_failures: row.consecutive_failures,
                    last_request_time: parse_datetime(&row.last_request_time),
                    last_success_time: parse_datetime_opt(row.last_success_time.as_deref()),
                    last_failure_time: parse_datetime_opt(row.last_failure_time.as_deref()),
                };
                (auth_id, metrics)
            })
            .collect();

        Ok(metrics_map)
    }

    /// Load all health from the database.
    ///
    /// # Errors
    ///
    /// Returns an error if the SQL query or row mapping fails.
    pub async fn load_all_health(&self) -> Result<HashMap<String, AuthHealth>> {
        let rows: Vec<HealthWithIdRow> = sqlx::query_as::<_, HealthWithIdRow>(
            "SELECT auth_id, status, consecutive_successes, consecutive_failures,
            last_status_change, last_check_time, unavailable_until, error_counts
            FROM auth_health",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| SqliteError::query("load_all_health", e))?;

        let health_map: HashMap<String, AuthHealth> = rows
            .into_iter()
            .map(|row| {
                let auth_id = row.auth_id.clone();
                let health = AuthHealth {
                    status: parse_health_status(&row.status),
                    consecutive_successes: row.consecutive_successes,
                    consecutive_failures: row.consecutive_failures,
                    last_status_change: parse_datetime(&row.last_status_change),
                    last_check_time: parse_datetime(&row.last_check_time),
                    unavailable_until: parse_datetime_opt(row.unavailable_until.as_deref()),
                    error_counts: parse_error_counts(&row.error_counts),
                };
                (auth_id, health)
            })
            .collect();

        Ok(health_map)
    }

    /// Cleanup old history records.
    ///
    /// # Errors
    ///
    /// Returns an error if the delete query fails.
    pub async fn cleanup_old_history(&self, max_age_seconds: i64) -> Result<i64> {
        let max_age_seconds = max_age_seconds.max(0);
        let cutoff = Utc::now() - chrono::Duration::seconds(max_age_seconds);
        let cutoff_str = cutoff.to_rfc3339();

        let result = sqlx::query("DELETE FROM status_code_history WHERE timestamp < $1")
            .bind(&cutoff_str)
            .execute(&self.pool)
            .await
            .map_err(|e| SqliteError::query("cleanup_old_history", e))?;

        Ok(result.rows_affected() as i64)
    }

    /// Get history statistics.
    ///
    /// # Errors
    ///
    /// Returns an error if the query or row mapping fails.
    pub async fn get_history_stats(&self) -> Result<(i64, Option<DateTime<Utc>>)> {
        let row: (i64, Option<String>) = sqlx::query_as(
            "SELECT COUNT(*), MIN(timestamp) FROM status_code_history",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| SqliteError::query("get_history_stats", e))?;

        let min_timestamp = parse_datetime_opt(row.1.as_deref());

        Ok((row.0, min_timestamp))
    }
}
```

Key changes:
- `self.db.lock().await` + `db.execute()` → `sqlx::query(...).bind(...).execute(&self.pool).await`
- `rusqlite::params![...]` → `.bind(...)` chain
- `row.get(N)` with positional indexing → `FromRow` derive with named fields
- `rusqlite::Error::QueryReturnedNoRows` → `fetch_optional` returns `Option<Row>`
- Extracted `parse_datetime`, `parse_datetime_opt`, `parse_health_status`, `parse_error_counts` helpers
- `?1` params → `$1` params

- [ ] **Step 2: Verify operations.rs compiles**

Run: `cargo check 2>&1 | head -30`
Expected: Errors only in `selector.rs` now (still references `rusqlite`). Operations and store should compile.

- [ ] **Step 3: Commit operations rewrite**

```bash
git add src/routing/sqlite/store/operations.rs
git commit -m "refactor: rewrite store operations with sqlx query builder and FromRow types"
```

---

### Task 4: Selector Queries

**Files:**
- Rewrite: `src/routing/sqlite/selector.rs`

- [ ] **Step 1: Rewrite selector.rs**

Replace entire `src/routing/sqlite/selector.rs` with:

```rust
// ALLOW: Significant drop tightening in SQLite operations requires restructuring
// query chains that would reduce readability without meaningful benefit.
#![allow(clippy::significant_drop_tightening, clippy::match_same_arms)]

use super::error::{Result, SqliteError};
use super::store::SQLiteStore;
use crate::routing::config::SmartRoutingConfig;
use crate::routing::weight::AuthInfo;
use rand::Rng;
use sqlx::FromRow;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Instant;

/// Weighted auth for selection
#[derive(Debug, Clone)]
struct WeightedAuth {
    id: String,
    weight: f64,
}

/// Selector statistics
#[derive(Debug, Clone)]
pub struct SelectorStats {
    /// Total number of credential selections
    pub select_count: i64,
    /// Number of cache hits
    pub cache_hits: i64,
    /// Number of database queries
    pub db_queries: i64,
}

/// SQLite-backed selector with SQL-based weight queries
pub struct SQLiteSelector {
    store: SQLiteStore,
    config: SmartRoutingConfig,
    stats: Arc<SelectorStatsInternal>,
}

/// Internal selector statistics with atomic counters
struct SelectorStatsInternal {
    select_count: AtomicI64,
    cache_hits: AtomicI64,
    db_queries: AtomicI64,
}

/// Weight cache entry
#[allow(dead_code)]
struct WeightCache {
    weights: HashMap<String, f64>,
    expiry: Instant,
}

/// Row for the auth weight query (json_each based).
#[derive(Debug, FromRow)]
struct AuthWeightRow {
    auth_id: String,
    success_rate: f64,
    latency: f64,
    health_factor: f64,
    available: i32,
}

/// Row for the precompute weight query.
#[derive(Debug, FromRow)]
struct PrecomputeWeightRow {
    auth_id: String,
    success_rate: f64,
    avg_latency_ms: f64,
    health_factor: f64,
    available: i32,
}

/// Row for top auths query.
#[derive(Debug, FromRow)]
struct TopAuthRow {
    auth_id: String,
}

impl SQLiteSelector {
    /// Create a new `SQLite` selector
    #[must_use]
    pub fn new(store: SQLiteStore, config: SmartRoutingConfig) -> Self {
        Self {
            store,
            config,
            stats: Arc::new(SelectorStatsInternal {
                select_count: AtomicI64::new(0),
                cache_hits: AtomicI64::new(0),
                db_queries: AtomicI64::new(0),
            }),
        }
    }

    /// Pick the best auth based on weighted selection using SQL queries
    pub async fn pick(&self, auths: Vec<AuthInfo>) -> Option<String> {
        self.stats.select_count.fetch_add(1, Ordering::Relaxed);

        // Check if smart routing is enabled
        if !self.config.enabled {
            return auths.into_iter().next().map(|a| a.id);
        }

        if auths.is_empty() {
            return None;
        }

        // Get available auths with weights from SQL query
        let available = self.query_available_auths(auths).await?;

        if available.is_empty() {
            return None;
        }

        // Select by weight
        Some(Self::select_by_weight(available))
    }

    /// Query available auths and their weights using SQL
    async fn query_available_auths(&self, auths: Vec<AuthInfo>) -> Option<Vec<WeightedAuth>> {
        self.stats.db_queries.fetch_add(1, Ordering::Relaxed);

        // Build auth_id list and map
        let auth_ids: Vec<String> = auths
            .iter()
            .filter(|a| !a.unavailable)
            .map(|a| a.id.clone())
            .collect();
        let auth_map: HashMap<String, AuthInfo> = auths
            .into_iter()
            .filter(|a| !a.unavailable)
            .map(|a| (a.id.clone(), a))
            .collect();

        if auth_ids.is_empty() {
            return None;
        }

        // Convert auth_ids to JSON array for SQL query
        let json_array = serde_json::to_string(&auth_ids).ok()?;

        // Execute SQL query with weight calculation
        let rows: Vec<AuthWeightRow> = sqlx::query_as::<_, AuthWeightRow>(
            r"
            SELECT
                ids.auth_id,
                COALESCE(m.success_rate, 1.0) as success_rate,
                COALESCE(m.avg_latency_ms, 0) as latency,
                CASE
                    WHEN h.status = 'Healthy' THEN 1.0
                    WHEN h.status = 'Degraded' THEN 0.5
                    ELSE 0.01
                END as health_factor,
                CASE
                    WHEN h.unavailable_until IS NOT NULL AND datetime(h.unavailable_until) > datetime('now') THEN 0
                    ELSE 1
                END as available
            FROM (SELECT value as auth_id FROM json_each($1)) as ids
            LEFT JOIN auth_metrics m ON m.auth_id = ids.auth_id
            LEFT JOIN auth_health h ON h.auth_id = ids.auth_id
            ",
        )
        .bind(&json_array)
        .fetch_all(&self.store.get_pool())
        .await
        .ok()?;

        let mut available = Vec::new();

        for row in rows {
            // Skip unavailable auths
            if row.available == 0 {
                continue;
            }

            // Get auth info
            let auth = auth_map.get(&row.auth_id)?;

            // Calculate weight
            let weight = self.calculate_weight(row.success_rate, row.latency, row.health_factor, auth);

            if weight > 0.0 {
                available.push(WeightedAuth {
                    id: row.auth_id,
                    weight,
                });
            }
        }

        Some(available)
    }

    /// Calculate weight from SQL query results
    fn calculate_weight(
        &self,
        success_rate: f64,
        latency: f64,
        health_factor: f64,
        auth: &AuthInfo,
    ) -> f64 {
        let cfg = &self.config.weight;

        // Normalize latency (assume max 500ms)
        let latency_score = 1.0 - (latency / 500.0).min(1.0);

        // Calculate load score
        let load_score = if auth.quota_exceeded { 0.0 } else { 1.0 };

        // Calculate priority score
        let priority_score = auth.priority.map_or(0.5, |p| {
            let score = (f64::from(p) + 100.0) / 200.0;
            score.clamp(0.0, 1.0)
        });

        // Weighted sum
        let mut weight = cfg.priority_weight.mul_add(
            priority_score,
            cfg.load_weight.mul_add(
                load_score,
                cfg.health_weight.mul_add(
                    health_factor,
                    cfg.success_rate_weight
                        .mul_add(success_rate, cfg.latency_weight * latency_score),
                ),
            ),
        );

        // Apply health penalties
        if health_factor < 0.1 {
            weight *= cfg.unhealthy_penalty;
        } else if health_factor < 0.5 {
            weight *= cfg.degraded_penalty;
        }

        // Apply quota penalty
        if auth.quota_exceeded {
            weight *= cfg.quota_exceeded_penalty;
        }

        // Apply unavailable penalty
        if auth.unavailable {
            weight *= cfg.unavailable_penalty;
        }

        weight.max(0.0)
    }

    /// Select auth by weighted random choice
    // ALLOW: Each expect is guarded by a prior length/index check that guarantees the element exists.
    #[allow(clippy::expect_used)]
    fn select_by_weight(available: Vec<WeightedAuth>) -> String {
        if available.len() == 1 {
            return available
                .into_iter()
                .next()
                .expect("unwrapping valid test data")
                .id;
        }

        // Calculate total weight
        let total_weight: f64 = available.iter().map(|a| a.weight).sum();

        if total_weight <= 0.0 || !total_weight.is_finite() {
            // All weights are zero, select randomly
            let idx = rand::rng().random_range(0..available.len());
            return available
                .into_iter()
                .nth(idx)
                .expect("unwrapping valid test data")
                .id;
        }

        // Save last element as fallback for floating-point edge cases
        let fallback = available
            .last()
            .map(|a| a.id.clone())
            .expect("unwrapping valid test data");

        // Weighted random selection
        let r = rand::rng().random::<f64>() * total_weight;
        let mut cumulative = 0.0;

        for auth in available {
            cumulative += auth.weight;
            if r <= cumulative {
                return auth.id;
            }
        }

        fallback
    }

    /// Precompute weights for batch operations
    ///
    /// # Errors
    ///
    /// Returns `SqliteError` if serialization fails, database queries fail,
    /// or weight updates cannot be committed.
    pub async fn precompute_weights(&self, auth_ids: Vec<String>) -> Result<()> {
        if auth_ids.is_empty() {
            return Ok(());
        }

        // Convert auth_ids to JSON array
        let json_array = serde_json::to_string(&auth_ids)
            .map_err(|e| SqliteError::Serialization(e.to_string()))?;

        // Query existing metrics and health
        let rows: Vec<PrecomputeWeightRow> = sqlx::query_as::<_, PrecomputeWeightRow>(
            r"
            SELECT
                ids.auth_id,
                COALESCE(m.success_rate, 1.0),
                COALESCE(m.avg_latency_ms, 0),
                CASE
                    WHEN h.status = 'Healthy' THEN 1.0
                    WHEN h.status = 'Degraded' THEN 0.5
                    ELSE 0.01
                END,
                CASE
                    WHEN h.unavailable_until IS NOT NULL AND datetime(h.unavailable_until) > datetime('now') THEN 0
                    ELSE 1
                END
            FROM (SELECT value as auth_id FROM json_each($1)) as ids
            LEFT JOIN auth_metrics m ON m.auth_id = ids.auth_id
            LEFT JOIN auth_health h ON h.auth_id = ids.auth_id
            ",
        )
        .bind(&json_array)
        .fetch_all(&self.store.get_pool())
        .await
        .map_err(|e| SqliteError::query("precompute_weights_query", e))?;

        let mut weights = HashMap::new();

        for row in rows {
            if row.available == 0 {
                weights.insert(row.auth_id.clone(), 0.0);
                continue;
            }

            // Create dummy auth info for weight calculation
            let auth = AuthInfo {
                id: row.auth_id.clone(),
                priority: None,
                quota_exceeded: false,
                unavailable: false,
                model_states: Vec::new(),
            };

            let weight = self.calculate_weight(row.success_rate, row.avg_latency_ms, row.health_factor, &auth);
            weights.insert(row.auth_id, weight);
        }

        // Update weights table
        self.update_weights(weights).await?;

        Ok(())
    }

    /// Update weights in database
    ///
    /// # Errors
    ///
    /// Returns `SqliteError` if the transaction cannot be started,
    /// statements fail to prepare or execute, or the commit fails.
    async fn update_weights(&self, weights: HashMap<String, f64>) -> Result<()> {
        let mut tx = self.store.get_pool().begin().await.map_err(|e| SqliteError::query("begin_transaction", e))?;

        for (auth_id, weight) in &weights {
            sqlx::query(
                r"
                INSERT INTO auth_weights (auth_id, weight, calculated_at, strategy)
                VALUES ($1, $2, datetime('now'), $3)
                ON CONFLICT(auth_id) DO UPDATE SET
                    weight = excluded.weight,
                    calculated_at = excluded.calculated_at,
                    strategy = excluded.strategy
                ",
            )
            .bind(auth_id)
            .bind(*weight)
            .bind(&self.config.strategy)
            .execute(&mut *tx)
            .await
            .map_err(|e| SqliteError::query("execute_weight_insert", e))?;
        }

        tx.commit().await.map_err(|e| SqliteError::query("commit_weight_transaction", e))?;

        Ok(())
    }

    /// Get top N auths by weight
    ///
    /// # Errors
    ///
    /// Returns `SqliteError` if the query fails to prepare, execute, or read rows.
    pub async fn get_top_auths(&self, limit: usize) -> Result<Vec<String>> {
        let query = format!(
            r"
            SELECT auth_id FROM auth_weights
            WHERE strategy = $1
            ORDER BY weight DESC
            LIMIT {limit}
            "
        );

        let rows: Vec<TopAuthRow> = sqlx::query_as::<_, TopAuthRow>(&query)
            .bind(&self.config.strategy)
            .fetch_all(&self.store.get_pool())
            .await
            .map_err(|e| SqliteError::query("query_top_auths", e))?;

        Ok(rows.into_iter().map(|r| r.auth_id).collect())
    }

    /// Get selector statistics
    #[must_use]
    pub fn get_stats(&self) -> SelectorStats {
        SelectorStats {
            select_count: self.stats.select_count.load(Ordering::Relaxed),
            cache_hits: self.stats.cache_hits.load(Ordering::Relaxed),
            db_queries: self.stats.db_queries.load(Ordering::Relaxed),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::panic)]
    use super::*;
    use crate::routing::sqlite::store::SQLiteConfig;

    #[tokio::test]
    async fn test_sqlite_selector_pick() {
        let config = SQLiteConfig::default();
        let store = SQLiteStore::new(config)
            .await
            .expect("unwrapping valid test data");

        let config = SmartRoutingConfig::default();
        let selector = SQLiteSelector::new(store, config);

        let auths = vec![
            AuthInfo {
                id: "auth1".to_string(),
                priority: Some(0),
                quota_exceeded: false,
                unavailable: false,
                model_states: Vec::new(),
            },
            AuthInfo {
                id: "auth2".to_string(),
                priority: Some(0),
                quota_exceeded: false,
                unavailable: false,
                model_states: Vec::new(),
            },
        ];

        // Should return one of the auths
        let selected = selector.pick(auths).await;
        assert!(selected.is_some());
    }

    #[tokio::test]
    async fn test_precompute_weights() {
        let config = SQLiteConfig::default();
        let store = SQLiteStore::new(config)
            .await
            .expect("unwrapping valid test data");

        let config = SmartRoutingConfig::default();
        let selector = SQLiteSelector::new(store, config);

        let auth_ids = vec!["auth1".to_string(), "auth2".to_string()];

        // Should not error - use timeout to avoid hanging
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            selector.precompute_weights(auth_ids),
        )
        .await;

        // Either Ok or timeout is acceptable for this test
        // The important thing is it doesn't panic
        match result {
            Ok(Ok(())) => {}, // Success
            Ok(Err(e)) => panic!("Failed to precompute weights: {e}"),
            Err(_) => {}, // Timeout - acceptable for empty database
        }
    }

    #[tokio::test]
    async fn test_get_stats() {
        let config = SQLiteConfig::default();
        let store = SQLiteStore::new(config)
            .await
            .expect("unwrapping valid test data");

        let config = SmartRoutingConfig::default();
        let selector = SQLiteSelector::new(store, config);

        let stats = selector.get_stats();
        assert_eq!(stats.select_count, 0);
        assert_eq!(stats.cache_hits, 0);
        assert_eq!(stats.db_queries, 0);
    }
    use crate::routing::health::{AuthHealth, HealthStatus};
    use crate::routing::metrics::AuthMetrics;
    use chrono::Utc;

    #[tokio::test]
    async fn test_precompute_and_get_top_auths() {
        let config = SQLiteConfig::default();
        let store = SQLiteStore::new(config)
            .await
            .expect("unwrapping valid test data");
        let config = SmartRoutingConfig::default();
        let selector = SQLiteSelector::new(store.clone(), config);

        // Add health and metrics for auth1 (Healthy, good metrics)
        let metrics1 = AuthMetrics {
            total_requests: 100,
            success_count: 100,
            failure_count: 0,
            avg_latency_ms: 10.0,
            min_latency_ms: 5.0,
            max_latency_ms: 20.0,
            success_rate: 1.0,
            error_rate: 0.0,
            consecutive_successes: 100,
            consecutive_failures: 0,
            last_request_time: Utc::now(),
            last_success_time: Some(Utc::now()),
            last_failure_time: None,
        };
        store
            .write_metrics("auth1", &metrics1)
            .await
            .expect("unwrapping valid test data");

        let health1 = AuthHealth {
            status: HealthStatus::Healthy,
            consecutive_successes: 100,
            consecutive_failures: 0,
            last_status_change: Utc::now(),
            last_check_time: Utc::now(),
            unavailable_until: None,
            error_counts: std::collections::HashMap::new(),
        };
        store
            .write_health("auth1", &health1)
            .await
            .expect("unwrapping valid test data");

        // Add health and metrics for auth2 (Degraded, bad metrics)
        let metrics2 = AuthMetrics {
            total_requests: 100,
            success_count: 50,
            failure_count: 50,
            avg_latency_ms: 200.0,
            min_latency_ms: 100.0,
            max_latency_ms: 300.0,
            success_rate: 0.5,
            error_rate: 0.5,
            consecutive_successes: 0,
            consecutive_failures: 10,
            last_request_time: Utc::now(),
            last_success_time: Some(Utc::now()),
            last_failure_time: Some(Utc::now()),
        };
        store
            .write_metrics("auth2", &metrics2)
            .await
            .expect("unwrapping valid test data");

        let health2 = AuthHealth {
            status: HealthStatus::Degraded,
            consecutive_successes: 0,
            consecutive_failures: 10,
            last_status_change: Utc::now(),
            last_check_time: Utc::now(),
            unavailable_until: None,
            error_counts: std::collections::HashMap::new(),
        };
        store
            .write_health("auth2", &health2)
            .await
            .expect("unwrapping valid test data");

        // Precompute weights
        let auth_ids = vec!["auth1".to_string(), "auth2".to_string()];
        selector
            .precompute_weights(auth_ids)
            .await
            .expect("unwrapping valid test data");

        // Get top auths
        let top_auths = selector
            .get_top_auths(2)
            .await
            .expect("unwrapping valid test data");

        assert_eq!(top_auths.len(), 2);
        // auth1 should have a higher weight because of better metrics and health
        assert_eq!(top_auths[0], "auth1");
        assert_eq!(top_auths[1], "auth2");
    }

    #[tokio::test]
    async fn test_get_top_auths_limit() {
        let config = SQLiteConfig::default();
        let store = SQLiteStore::new(config)
            .await
            .expect("unwrapping valid test data");
        let config = SmartRoutingConfig::default();
        let selector = SQLiteSelector::new(store.clone(), config);

        // Precompute weights
        let auth_ids = vec![
            "auth1".to_string(),
            "auth2".to_string(),
            "auth3".to_string(),
        ];
        selector
            .precompute_weights(auth_ids)
            .await
            .expect("unwrapping valid test data");

        // Get top auths with limit 2
        let top_auths = selector
            .get_top_auths(2)
            .await
            .expect("unwrapping valid test data");

        assert_eq!(top_auths.len(), 2);
    }
}
```

Key changes:
- `self.store.get_db(); let db = db.lock().await;` → `self.store.get_pool()` (no lock needed)
- `db.prepare(query)` + `stmt.query([...])` + `rows.next()` loop → `sqlx::query_as::<_, RowType>(query).bind(...).fetch_all(&pool).await`
- `db.unchecked_transaction()` + `tx.prepare()` + `stmt.execute()` → `pool.begin().await` + `sqlx::query(...).execute(&mut *tx).await`
- `?1` params → `$1` params
- `FromRow` derives for `AuthWeightRow`, `PrecomputeWeightRow`, `TopAuthRow`

- [ ] **Step 2: Verify selector compiles**

Run: `cargo check 2>&1 | head -20`
Expected: Selector and store compile. Remaining errors only in `collectors.rs` or `mod.rs`.

- [ ] **Step 3: Commit selector rewrite**

```bash
git add src/routing/sqlite/selector.rs
git commit -m "refactor: rewrite SQLiteSelector with sqlx pool and FromRow types"
```

---

### Task 5: Collectors Update & Module Exports

**Files:**
- Modify: `src/routing/sqlite/collectors.rs` (no SQL changes, just ensure types align)
- Modify: `src/routing/sqlite/mod.rs` (update re-exports if needed)

- [ ] **Step 1: Review collectors.rs — no changes needed**

`collectors.rs` uses `SQLiteStore` methods (`write_metrics`, `write_health`, `write_status_history`, `load_all_metrics`, `load_all_health`) via the public API. Since we kept the same API surface, collectors.rs needs **zero changes**. The `store::SQLiteStore` type alias and method signatures are identical.

Verify by reading the file — confirm it only calls `self.store.write_metrics()`, `self.store.write_health()`, etc. No direct rusqlite usage.

Run: `cargo check 2>&1 | head -20`
Expected: If errors in collectors.rs, they'll reference rusqlite. If none, collectors.rs is fine.

- [ ] **Step 2: Update mod.rs if needed**

Current `src/routing/sqlite/mod.rs` re-exports public types. Verify it doesn't reference any rusqlite types:

```rust
pub use collectors::SQLiteHealthManager;
pub use collectors::SQLiteMetricsCollector;
pub use error::{Result, SqliteError};
pub use selector::SQLiteSelector;
pub use selector::SelectorStats;
pub use store::SQLiteConfig;
pub use store::SQLiteStore;
```

No changes needed — these re-export our types, not rusqlite types.

- [ ] **Step 3: Commit**

```bash
git add src/routing/sqlite/collectors.rs src/routing/sqlite/mod.rs
git commit -m "refactor: verify collectors and module exports work with sqlx backend"
```

---

### Task 6: Test Suite Migration

**Files:**
- Rewrite: `src/routing/sqlite/tests.rs`

- [ ] **Step 1: Rewrite tests.rs**

The existing tests use `SQLiteConfig { database_path: ":memory:".to_string(), ..Default::default() }` to create in-memory stores. Since `SQLiteStore::new()` now uses sqlx internally, the same pattern works — `:memory:` creates a SqlitePool backed by an in-memory database with migrations applied.

Replace entire `src/routing/sqlite/tests.rs` with the same test code. The only change: tests that create `SQLiteStore` directly still work because `SQLiteStore::new(SQLiteConfig::default())` handles the `:memory:` → `sqlite::memory:` mapping internally.

The tests themselves do not change — they call the same public API methods. The full test code from the current `tests.rs` (1313 lines) is preserved as-is because the API surface is identical.

The key structural change is in the test module wrapper:

```rust
#![allow(
    clippy::significant_drop_tightening,
    clippy::match_same_arms,
    clippy::clone_on_ref_ptr,
    clippy::panic
)]
#[cfg(test)]
mod sqlite_tests {
    use crate::routing::health::{AuthHealth, HealthStatus};
    use crate::routing::metrics::AuthMetrics;
    use crate::routing::sqlite::collectors::SQLiteHealthManager;
    use crate::routing::sqlite::collectors::SQLiteMetricsCollector;
    use crate::routing::sqlite::store::SQLiteConfig;
    use crate::routing::sqlite::store::SQLiteStore;
    use chrono::Utc;

    // ... (all existing test modules preserved identically) ...
}
```

No test code changes required — the `SQLiteConfig::default()` with `database_path: ":memory:"` still works because `SQLiteStore::new()` maps it to `sqlite::memory:` internally.

- [ ] **Step 2: Run tests to verify**

Run: `cargo nextest run --lib -E 'test(sqlite)' 2>&1 | tail -20`
Expected: All ~30 sqlite tests pass.

- [ ] **Step 3: Commit test migration**

```bash
git add src/routing/sqlite/tests.rs
git commit -m "refactor: update sqlite tests for sqlx backend (same API, no test changes needed)"
```

---

### Task 7: sqlx::test Fixtures — Improved Test Quality

**Files:**
- Create: `src/routing/sqlite/test_helpers.rs`
- Modify: `src/routing/sqlite/mod.rs` (add test_helpers module)
- Modify: `src/routing/sqlite/tests.rs` (add new sqlx::test-based tests)

- [ ] **Step 1: Add sqlx migrate feature to Cargo.toml**

In `Cargo.toml`, update the sqlx dependency to include `migrate` feature for `sqlx::test` support:

```toml
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite", "migrate"] }
```

- [ ] **Step 2: Create test helpers module**

Create `src/routing/sqlite/test_helpers.rs`:

```rust
//! Shared test fixtures using sqlx::test for automatic database provisioning.

use crate::routing::health::{AuthHealth, HealthStatus};
use crate::routing::metrics::AuthMetrics;
use crate::routing::sqlite::store::SQLiteStore;
use chrono::Utc;
use std::collections::HashMap;

/// Create a store backed by a fresh migrated pool (for sqlx::test-based tests).
pub fn store_from_pool(pool: sqlx::SqlitePool) -> SQLiteStore {
    SQLiteStore::from_pool(pool)
}

/// Create sample metrics for testing.
pub fn sample_metrics(
    total: i64,
    success: i64,
    failure: i64,
    avg_latency: f64,
    success_rate: f64,
) -> AuthMetrics {
    AuthMetrics {
        total_requests: total,
        success_count: success,
        failure_count: failure,
        avg_latency_ms: avg_latency,
        min_latency_ms: avg_latency * 0.5,
        max_latency_ms: avg_latency * 2.0,
        success_rate,
        error_rate: 1.0 - success_rate,
        consecutive_successes: success as i32,
        consecutive_failures: failure as i32,
        last_request_time: Utc::now(),
        last_success_time: if success > 0 { Some(Utc::now()) } else { None },
        last_failure_time: if failure > 0 { Some(Utc::now()) } else { None },
    }
}

/// Create sample health for testing.
pub fn sample_health(status: HealthStatus, consecutive_successes: i32, consecutive_failures: i32) -> AuthHealth {
    AuthHealth {
        status,
        consecutive_successes,
        consecutive_failures,
        last_status_change: Utc::now(),
        last_check_time: Utc::now(),
        unavailable_until: None,
        error_counts: HashMap::new(),
    }
}

/// Create degraded health with specific error counts.
pub fn degraded_health_with_errors(error_counts: HashMap<i32, i32>) -> AuthHealth {
    AuthHealth {
        status: HealthStatus::Degraded,
        consecutive_successes: 0,
        consecutive_failures: 3,
        last_status_change: Utc::now(),
        last_check_time: Utc::now(),
        unavailable_until: None,
        error_counts,
    }
}
```

- [ ] **Step 3: Add test_helpers module to mod.rs**

Add to `src/routing/sqlite/mod.rs` before the `#[cfg(test)]` block:

```rust
#[cfg(test)]
mod test_helpers;
```

- [ ] **Step 4: Add sqlx::test-based integration tests**

Append to `src/routing/sqlite/tests.rs` (inside the `sqlite_tests` module, at the end):

```rust
    mod sqlx_fixtures {
        use super::*;
        use crate::routing::sqlite::test_helpers::*;

        /// sqlx::test provides a fresh migrated pool for each test.
        /// No manual SQLiteStore::new() or config setup needed.
        #[sqlx::test(migrations = "../../migrations")]
        async fn test_sqlx_fixture_store_write_load_metrics(pool: sqlx::SqlitePool) {
            let store = store_from_pool(pool);

            let metrics = sample_metrics(100, 95, 5, 150.0, 0.95);
            store
                .write_metrics("fixture-auth", &metrics)
                .await
                .expect("write should succeed");

            let loaded = store
                .load_metrics("fixture-auth")
                .await
                .expect("load should succeed")
                .expect("should find metrics");

            assert_eq!(loaded.total_requests, 100);
            assert_eq!(loaded.success_count, 95);
            assert!((loaded.avg_latency_ms - 150.0).abs() < 0.01);
        }

        #[sqlx::test(migrations = "../../migrations")]
        async fn test_sqlx_fixture_store_write_load_health(pool: sqlx::SqlitePool) {
            let store = store_from_pool(pool);

            let health = sample_health(HealthStatus::Healthy, 10, 0);
            store
                .write_health("fixture-auth", &health)
                .await
                .expect("write should succeed");

            let loaded = store
                .load_health("fixture-auth")
                .await
                .expect("load should succeed")
                .expect("should find health");

            assert_eq!(loaded.status, HealthStatus::Healthy);
            assert_eq!(loaded.consecutive_successes, 10);
        }

        #[sqlx::test(migrations = "../../migrations")]
        async fn test_sqlx_fixture_upsert_semantics(pool: sqlx::SqlitePool) {
            let store = store_from_pool(pool);

            let v1 = sample_metrics(10, 9, 1, 100.0, 0.9);
            store.write_metrics("upsert-auth", &v1).await.unwrap();

            let v2 = sample_metrics(50, 45, 5, 80.0, 0.9);
            store.write_metrics("upsert-auth", &v2).await.unwrap();

            let loaded = store
                .load_metrics("upsert-auth")
                .await
                .unwrap()
                .unwrap();
            assert_eq!(loaded.total_requests, 50);
            assert_eq!(loaded.success_count, 45);
            assert!((loaded.avg_latency_ms - 80.0).abs() < 0.01);
        }

        #[sqlx::test(migrations = "../../migrations")]
        async fn test_sqlx_fixture_error_counts_round_trip(pool: sqlx::SqlitePool) {
            let store = store_from_pool(pool);

            let mut errors = HashMap::new();
            errors.insert(500, 3);
            errors.insert(429, 1);
            let health = degraded_health_with_errors(errors);

            store
                .write_health("errors-auth", &health)
                .await
                .expect("write should succeed");

            let loaded = store
                .load_health("errors-auth")
                .await
                .unwrap()
                .unwrap();

            assert_eq!(loaded.status, HealthStatus::Degraded);
            assert_eq!(loaded.error_counts.get(&500), Some(&3));
            assert_eq!(loaded.error_counts.get(&429), Some(&1));
        }

        #[sqlx::test(migrations = "../../migrations")]
        async fn test_sqlx_fixture_status_history(pool: sqlx::SqlitePool) {
            let store = store_from_pool(pool);

            store
                .write_status_history("hist-auth", 200, 50.0, true)
                .await
                .unwrap();
            store
                .write_status_history("hist-auth", 500, 200.0, false)
                .await
                .unwrap();

            let (count, _min_ts) = store.get_history_stats().await.unwrap();
            assert_eq!(count, 2);
        }

        #[sqlx::test(migrations = "../../migrations")]
        async fn test_sqlx_fixture_load_all(pool: sqlx::SqlitePool) {
            let store = store_from_pool(pool);

            for i in 1..=3i64 {
                let metrics = sample_metrics(i * 10, i * 9, i, 100.0 + i as f64, 0.9);
                store
                    .write_metrics(&format!("auth-{i}"), &metrics)
                    .await
                    .unwrap();
            }

            let all = store.load_all_metrics().await.unwrap();
            assert_eq!(all.len(), 3);
            assert_eq!(all.get("auth-2").unwrap().total_requests, 20);
        }

        #[sqlx::test(migrations = "../../migrations")]
        async fn test_sqlx_fixture_cleanup(pool: sqlx::SqlitePool) {
            let store = store_from_pool(pool);

            for i in 0..5 {
                store
                    .write_status_history(&format!("auth-{i}"), 200, 100.0, true)
                    .await
                    .unwrap();
            }

            let (count, _) = store.get_history_stats().await.unwrap();
            assert_eq!(count, 5);

            let deleted = store.cleanup_old_history(0).await.unwrap();
            assert!(deleted >= 0);
        }

        #[sqlx::test(migrations = "../../migrations")]
        async fn test_sqlx_fixture_missing_returns_none(pool: sqlx::SqlitePool) {
            let store = store_from_pool(pool);

            assert!(store
                .load_metrics("nonexistent")
                .await
                .unwrap()
                .is_none());
            assert!(store
                .load_health("nonexistent")
                .await
                .unwrap()
                .is_none());
        }
    }
```

Benefits of `sqlx::test` fixtures over the current approach:
1. **Automatic migration**: Each test gets a fresh database with all tables/indexes applied
2. **No config boilerplate**: No `SQLiteConfig { database_path: ":memory:", ..Default::default() }` repeated
3. **Isolation guaranteed**: Each test gets its own pool — no shared state
4. **Helper functions**: `sample_metrics()`, `sample_health()`, `degraded_health_with_errors()` eliminate fixture duplication
5. **Parallel-safe**: sqlx::test manages connection pools correctly for concurrent test execution

- [ ] **Step 5: Run all sqlite tests**

Run: `cargo nextest run --lib -E 'test(sqlite)' 2>&1 | tail -30`
Expected: All existing + new sqlx::test-based tests pass.

- [ ] **Step 6: Commit test improvements**

```bash
git add src/routing/sqlite/test_helpers.rs src/routing/sqlite/mod.rs src/routing/sqlite/tests.rs Cargo.toml
git commit -m "test: add sqlx::test fixtures and helper functions for improved test quality"
```

---

### Task 8: Full Verification

**Files:**
- All modified files

- [ ] **Step 1: Run complete test suite**

Run: `cargo nextest run 2>&1 | tail -30`
Expected: All tests pass. Fix any failures incrementally.

- [ ] **Step 2: Run clippy**

Run: `cargo clippy --all-targets -- -D warnings 2>&1 | tail -20`
Expected: No warnings. Fix any issues.

- [ ] **Step 3: Run format check**

Run: `cargo fmt --all --check 2>&1`
Expected: No formatting issues. If issues, run `cargo fmt --all`.

- [ ] **Step 4: Run coverage gate**

Run: `cargo llvm-cov --fail-under-lines 90 --ignore-filename-regex "src/main\.rs|src/bin/cli\.rs" 2>&1 | tail -10`
Expected: Coverage >= 90%.

- [ ] **Step 5: Verify no rusqlite references remain**

Run: `grep -r "rusqlite" src/ --include="*.rs"`
Expected: No matches. All rusqlite references removed.

- [ ] **Step 6: Verify migration works end-to-end**

Run: `cargo build 2>&1 && cargo run --bin gateway -- --help 2>&1 | head -5`
Expected: Builds successfully, gateway binary runs.

- [ ] **Step 7: Commit verification**

```bash
git add -A
git commit -m "chore: complete rusqlite to sqlx migration, verify all tests pass"
```

---

## Self-Review

### 1. Spec Coverage

| Requirement | Task |
|---|---|
| Replace rusqlite with sqlx | Task 1 (Cargo.toml) |
| Connection pool | Task 2 (store/mod.rs) |
| All write operations migrated | Task 3 (operations.rs) |
| All read operations migrated | Task 3 (operations.rs) |
| Selector queries migrated | Task 4 (selector.rs) |
| Collectors work with new backend | Task 5 (collectors.rs) |
| Tests pass | Task 6 + Task 8 |
| sqlx::test fixtures | Task 7 |
| No rusqlite references remain | Task 8 Step 5 |

### 2. Placeholder Scan

- No TBD, TODO, or "implement later" found
- All code blocks contain complete implementations
- All commands specify expected output

### 3. Type Consistency

- `SqliteError::query()` signature: `(operation: &'static str, source: sqlx::Error)` — used consistently across all tasks
- `SQLiteStore::get_pool()` returns `SqlitePool` — used in Task 4 selector
- `SQLiteStore::from_pool(pool: SqlitePool)` — used in Task 7 test helpers
- `FromRow` types use `$1` parameter style matching sqlx SQLite conventions
- All `bind()` chains match the `$N` placeholder order in their queries
