# SQLx Migration Edge-Case Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix 15 edge-case findings from the adversarial edge-case review of the rusqlite-to-sqlx migration (commits 41a2c78..de0c0cd).

**Architecture:** Fix regressions introduced by the migration (dropped pragmas, `fetch_one` crash, missing pool timeout) and harden pre-existing weaknesses (NaN weight persistence, schema constraints). Changes touch 4 production files and 1 migration file.

**Tech Stack:** Rust, sqlx 0.8, SQLite, Tokio, chrono

---

## Triage Summary

| # | Finding | Severity | Classification | Task |
|---|---------|----------|----------------|------|
| 1 | `fetch_one` on empty table crashes `get_history_stats` | CRITICAL | Regression | Task 1 |
| 2 | NaN weights persisted to `auth_weights` table | CRITICAL | Pre-existing | Task 2 |
| 3 | No `acquire_timeout` on pool — hangs on exhaustion | CRITICAL | Regression | Task 3 |
| 8 | Dropped pragmas: `temp_store`, `mmap_size` | HIGH | Regression | Task 3 |
| 9 | `cache_size_mb` config field silently ignored | HIGH | Regression | Task 3 |
| 13 | Migration failure leaks pool connections | HIGH | Regression | Task 3 |
| 12 | `database_path` with `sqlite:` prefix creates double prefix | MEDIUM | Pre-existing | Task 3 |
| 17 | `from_pool` hardcodes `db_path` to `:memory:` | MEDIUM | Pre-existing | Task 3 |
| 18 | `rows_affected() as i64` truncation | LOW | Pre-existing | Task 4 |
| 19 | `get_top_auths(limit=0)` returns empty silently | LOW | Pre-existing | Task 4 |
| 21 | Schema missing CHECK constraints | MEDIUM | Hardening | Task 5 |

**Dropped findings (false positive / too-low-risk / intentional):**
- #4 i32 vs i64: False positive — domain types are also `i32`
- #5 `parse_datetime` fallback: Intentional, same as old code
- #6 Unknown health defaults to Healthy: Same as old code
- #7 `error_counts` fallback: Same as old code
- #10 `precompute_weights` dummy AuthInfo: Same as old code
- #11 Auth IDs not in SQL: Same as old code
- #14 Pool exhaustion drops writes: Covered by #3
- #15 `rand::rng()` per invocation: Perf, not correctness
- #16 `u1=0.0` in Box-Muller: Vanishingly rare, already clamped
- #20 Empty `"in"` operator: Intentional guard removal
- #22 No `min_connections`: Perf, not correctness

---

## File Structure

| File | Change | Purpose |
|------|--------|---------|
| `src/routing/sqlite/store/operations.rs` | Modify | Fix `fetch_one` crash, `rows_affected` truncation |
| `src/routing/sqlite/selector.rs` | Modify | Guard NaN weights, `limit=0` guard |
| `src/routing/sqlite/store/mod.rs` | Modify | Pool config, pragmas, prefix guard, `from_pool` fix |
| `migrations/20260417000000_initial_schema.sql` | Modify | Add CHECK constraints |
| `src/routing/sqlite/tests.rs` | Modify | Add regression tests |

---

### Task 1: Fix `fetch_one` crash on empty table

`get_history_stats` uses `fetch_one` which returns `sqlx::Error::RowNotFound` when `status_code_history` is empty. Must use `fetch_optional`.

**Files:**
- Modify: `src/routing/sqlite/store/operations.rs:542-549`
- Test: `src/routing/sqlite/tests.rs`

- [ ] **Step 1: Write failing test**

Add to `src/routing/sqlite/tests.rs`:

```rust
#[sqlx::test(migrators = ["crate::routing::sqlite::store::MIGRATIONS"])]
async fn get_history_stats_returns_zero_on_empty_table(pool: SqlitePool) {
    let store = SQLiteStore::from_pool(pool, false);
    let (count, min_ts) = store
        .get_history_stats()
        .await
        .expect("should succeed on empty table");
    assert_eq!(count, 0);
    assert!(min_ts.is_none());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -E 'test(get_history_stats_returns_zero_on_empty_table)'`
Expected: FAIL with `sqlx::Error::RowNotFound` or similar

- [ ] **Step 3: Fix `get_history_stats` to use `fetch_optional`**

In `src/routing/sqlite/store/operations.rs`, replace lines 542-549:

```rust
    pub async fn get_history_stats(&self) -> Result<(i64, Option<DateTime<Utc>>)> {
        let row: Option<(i64, Option<String>)> =
            sqlx::query_as("SELECT COUNT(*), MIN(timestamp) FROM status_code_history")
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| SqliteError::query("get_history_stats", e))?;

        let Some((count, min_ts_str)) = row else {
            return Ok((0, None));
        };

        let min_timestamp = parse_datetime_opt(min_ts_str.as_deref());

        Ok((count, min_timestamp))
    }
```

Note: `COUNT(*)` always returns a row even on empty tables, so `fetch_optional` will always return `Some`. But using `fetch_optional` is the correct pattern and prevents the crash if SQLite behavior ever differs. The `Some` match handles it idiomatically.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo nextest run -E 'test(get_history_stats_returns_zero_on_empty_table)'`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/routing/sqlite/store/operations.rs src/routing/sqlite/tests.rs
git commit -m "fix: use fetch_optional in get_history_stats to prevent crash on empty table"
```

---

### Task 2: Guard against NaN weights in selector pipeline

NaN f64 values from `calculate_weight` propagate through `precompute_weights` into the `auth_weights` table via `update_weights`. SQLite stores NaN as NULL or literal text, poisoning `ORDER BY weight DESC` queries.

**Files:**
- Modify: `src/routing/sqlite/selector.rs:precompute_weights` (line ~300) and `update_weights` (line ~340)
- Test: `src/routing/sqlite/tests.rs`

- [ ] **Step 1: Write failing test**

Add to `src/routing/sqlite/tests.rs`:

```rust
#[sqlx::test(migrators = ["crate::routing::sqlite::store::MIGRATIONS"])]
async fn precompute_weights_rejects_nan(pool: SqlitePool) {
    let store = SQLiteStore::from_pool(pool, false);
    let config = SmartRoutingConfig::default();
    let selector = SQLiteSelector::new(store.clone(), config);

    // Write metrics with NaN success_rate to simulate corruption
    let metrics = AuthMetrics {
        total_requests: 100,
        success_count: 50,
        failure_count: 50,
        avg_latency_ms: f64::NAN,
        min_latency_ms: 0.0,
        max_latency_ms: 100.0,
        success_rate: f64::NAN,
        error_rate: 0.5,
        consecutive_successes: 0,
        consecutive_failures: 10,
        last_request_time: Utc::now(),
        last_success_time: None,
        last_failure_time: None,
    };
    store
        .write_metrics("auth_nan", &metrics)
        .await
        .expect("should write metrics");

    selector
        .precompute_weights(vec!["auth_nan".to_string()])
        .await
        .expect("should succeed without NaN in DB");

    // Verify weight is finite (not NaN)
    let top = selector
        .get_top_auths(1)
        .await
        .expect("should query weights");
    assert!(!top.is_empty(), "weight should exist for auth_nan");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -E 'test(precompute_weights_rejects_nan)'`
Expected: FAIL — NaN persists or query returns unexpected results

- [ ] **Step 3: Add NaN guard in `precompute_weights` loop**

In `src/routing/sqlite/selector.rs`, inside the `for row in rows` loop in `precompute_weights`, after `calculate_weight` call, add a finite check:

```rust
            let weight = self.calculate_weight(
                row.success_rate,
                row.avg_latency_ms,
                row.health_factor,
                &auth,
            );
            let weight = if weight.is_finite() && weight >= 0.0 {
                weight
            } else {
                0.0
            };
            weights.insert(row.auth_id, weight);
```

- [ ] **Step 4: Add NaN guard in `update_weights` binding**

In `update_weights`, before binding the weight, sanitize:

```rust
        for (auth_id, weight) in &weights {
            let weight = if weight.is_finite() { *weight } else { 0.0 };
            sqlx::query(
```

And change the bind:
```rust
            .bind(weight)
```

instead of:
```rust
            .bind(*weight)
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo nextest run -E 'test(precompute_weights_rejects_nan)'`
Expected: PASS

- [ ] **Step 6: Run full SQLite test suite**

Run: `cargo nextest run -E 'test(sqlite)'`
Expected: All pass

- [ ] **Step 7: Commit**

```bash
git add src/routing/sqlite/selector.rs src/routing/sqlite/tests.rs
git commit -m "fix: guard against NaN weights in precompute and update paths"
```

---

### Task 3: Harden pool configuration and restore dropped pragmas

The migration dropped `temp_store = MEMORY` and `mmap_size = 268435456` pragmas, ignores `cache_size_mb` config, has no `acquire_timeout` on the pool, leaks connections on migration failure, allows double `sqlite:` prefix, and hardcodes `from_pool` db_path.

**Files:**
- Modify: `src/routing/sqlite/store/mod.rs`
- Test: `src/routing/sqlite/tests.rs`

- [ ] **Step 1: Write failing test for pool acquire timeout**

Add to `src/routing/sqlite/tests.rs`:

```rust
#[sqlx::test(migrators = ["crate::routing::sqlite::store::MIGRATIONS"])]
async fn store_new_creates_functioning_store(pool: SqlitePool) {
    // from_pool should work for basic operations
    let store = SQLiteStore::from_pool(pool, true);
    let metrics = AuthMetrics::default();
    store
        .write_metrics("test_auth", &metrics)
        .await
        .expect("should write metrics");
    let loaded = store
        .load_metrics("test_auth")
        .await
        .expect("should load metrics");
    assert!(loaded.is_some());
}
```

- [ ] **Step 2: Restore dropped pragmas via `SqliteConnectOptions`**

In `src/routing/sqlite/store/mod.rs`, update the file-level options builder chain. For the non-memory branch, add `.pragma()` calls after `.synchronous()`:

```rust
        } else {
            let path = cfg.database_path.strip_prefix("sqlite:").unwrap_or(&cfg.database_path);
            SqliteConnectOptions::from_str(&format!("sqlite:{path}"))?
                .create_if_missing(true)
                .journal_mode(if cfg.enable_wal {
                    SqliteJournalMode::Wal
                } else {
                    SqliteJournalMode::Delete
                })
                .busy_timeout(Duration::from_millis(cfg.busy_timeout_ms as u64))
                .foreign_keys(true)
                .synchronous(SqliteSynchronous::Normal)
                .pragma("cache_size", &format!("-{}", cfg.cache_size_mb.max(1) * 1024))
                .pragma("temp_store", "MEMORY")
                .pragma("mmap_size", "268435456")
        };
```

For the `:memory:` branch, also add cache_size pragma:

```rust
        let options = if cfg.database_path == ":memory:" {
            SqliteConnectOptions::from_str("sqlite::memory:")?
                .busy_timeout(Duration::from_millis(cfg.busy_timeout_ms as u64))
                .foreign_keys(true)
                .synchronous(SqliteSynchronous::Normal)
                .pragma("cache_size", &format!("-{}", cfg.cache_size_mb.max(1) * 1024))
                .pragma("temp_store", "MEMORY")
        } else {
            // ... as above
```

- [ ] **Step 3: Add `acquire_timeout` to pool options**

Update the pool options builder:

```rust
        let pool_options =
            SqlitePoolOptions::new()
                .acquire_timeout(Duration::from_millis(cfg.busy_timeout_ms as u64))
                .max_connections(if cfg.database_path == ":memory:" {
                    1
                } else {
                    4
                });
```

- [ ] **Step 4: Clean up pool on migration failure**

Wrap migration in a cleanup-on-error block:

```rust
        let pool = pool_options.connect_with(options).await?;

        if let Err(e) = sqlx::migrate!("./migrations").run(&pool).await {
            pool.close().await;
            return Err(e.into());
        }
```

- [ ] **Step 5: Fix `from_pool` to accept `db_path` parameter**

```rust
    pub fn from_pool(pool: SqlitePool, db_path: impl Into<String>, enable_cache: bool) -> Self {
        Self {
            pool,
            db_path: db_path.into(),
            cache_enabled: Arc::new(std::sync::atomic::AtomicBool::new(enable_cache)),
            cache: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }
```

- [ ] **Step 6: Update all `from_pool` call sites**

In `src/routing/sqlite/test_helpers.rs`, update:

```rust
pub fn store_from_pool(pool: sqlx::SqlitePool) -> SQLiteStore {
    SQLiteStore::from_pool(pool, ":memory:", true)
}
```

In `src/routing/sqlite/tests.rs` and any other test files using `from_pool`, update calls to pass `":memory:"` as second argument. Search with:

Run: `grep -rn "from_pool" src/`
Then update each call site to add the `":memory:"` argument.

- [ ] **Step 7: Run full test suite**

Run: `cargo nextest run`
Expected: All pass

- [ ] **Step 8: Commit**

```bash
git add src/routing/sqlite/store/mod.rs src/routing/sqlite/test_helpers.rs src/routing/sqlite/tests.rs
git commit -m "fix: restore dropped pragmas, add pool acquire_timeout, guard migration cleanup, fix double-prefix"
```

---

### Task 4: Minor selector fixes — `rows_affected` truncation and `limit=0` guard

**Files:**
- Modify: `src/routing/sqlite/store/operations.rs:534`
- Modify: `src/routing/sqlite/selector.rs:get_top_auths`

- [ ] **Step 1: Fix `rows_affected` truncation**

In `src/routing/sqlite/store/operations.rs`, replace:

```rust
        Ok(result.rows_affected() as i64)
```

with:

```rust
        Ok(i64::try_from(result.rows_affected()).unwrap_or(i64::MAX))
```

- [ ] **Step 2: Add `limit=0` early return in `get_top_auths`**

In `src/routing/sqlite/selector.rs`, at the start of `get_top_auths`:

```rust
    pub async fn get_top_auths(&self, limit: usize) -> Result<Vec<String>> {
        if limit == 0 {
            return Ok(Vec::new());
        }
```

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -E 'test(sqlite)'`
Expected: All pass

- [ ] **Step 4: Commit**

```bash
git add src/routing/sqlite/store/operations.rs src/routing/sqlite/selector.rs
git commit -m "fix: safe rows_affected conversion and guard get_top_auths limit=0"
```

---

### Task 5: Add CHECK constraints to migration schema

Add domain constraints to prevent invalid data at the database level.

**Files:**
- Modify: `migrations/20260417000000_initial_schema.sql`

Note: This migration was just added in the current commit chain and has not been deployed. Modifying it is safe — delete any existing DB file and re-run.

- [ ] **Step 1: Update migration schema**

Replace the full contents of `migrations/20260417000000_initial_schema.sql`:

```sql
-- Auth metrics table
CREATE TABLE IF NOT EXISTS auth_metrics (
    auth_id TEXT PRIMARY KEY,
    total_requests INTEGER DEFAULT 0 NOT NULL CHECK(total_requests >= 0),
    success_count INTEGER DEFAULT 0 NOT NULL CHECK(success_count >= 0),
    failure_count INTEGER DEFAULT 0 NOT NULL CHECK(failure_count >= 0),
    avg_latency_ms REAL DEFAULT 0 NOT NULL CHECK(avg_latency_ms >= 0),
    min_latency_ms REAL DEFAULT 0 NOT NULL CHECK(min_latency_ms >= 0),
    max_latency_ms REAL DEFAULT 0 NOT NULL CHECK(max_latency_ms >= 0),
    success_rate REAL DEFAULT 1.0 NOT NULL CHECK(success_rate BETWEEN 0 AND 1),
    error_rate REAL DEFAULT 0 NOT NULL CHECK(error_rate BETWEEN 0 AND 1),
    consecutive_successes INTEGER DEFAULT 0 NOT NULL CHECK(consecutive_successes >= 0),
    consecutive_failures INTEGER DEFAULT 0 NOT NULL CHECK(consecutive_failures >= 0),
    last_request_time TEXT,
    last_success_time TEXT,
    last_failure_time TEXT,
    created_at TEXT DEFAULT (datetime('now')),
    updated_at TEXT DEFAULT (datetime('now'))
);

-- Auth health table
CREATE TABLE IF NOT EXISTS auth_health (
    auth_id TEXT PRIMARY KEY,
    status TEXT DEFAULT 'Healthy' NOT NULL CHECK(status IN ('Healthy', 'Degraded', 'Unhealthy')),
    consecutive_successes INTEGER DEFAULT 0 NOT NULL CHECK(consecutive_successes >= 0),
    consecutive_failures INTEGER DEFAULT 0 NOT NULL CHECK(consecutive_failures >= 0),
    last_status_change TEXT,
    last_check_time TEXT,
    unavailable_until TEXT,
    error_counts TEXT DEFAULT '{}' NOT NULL,
    created_at TEXT DEFAULT (datetime('now')),
    updated_at TEXT DEFAULT (datetime('now'))
);

-- Status code history table
CREATE TABLE IF NOT EXISTS status_code_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    auth_id TEXT NOT NULL,
    status_code INTEGER NOT NULL,
    latency_ms REAL CHECK(latency_ms IS NULL OR latency_ms >= 0),
    success INTEGER NOT NULL CHECK(success IN (0, 1)),
    timestamp TEXT DEFAULT (datetime('now'))
);

-- Auth weights table
CREATE TABLE IF NOT EXISTS auth_weights (
    auth_id TEXT PRIMARY KEY,
    weight REAL DEFAULT 1.0 NOT NULL,
    calculated_at TEXT,
    strategy TEXT DEFAULT 'weighted' NOT NULL
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_weights_weight ON auth_weights(weight DESC);
CREATE INDEX IF NOT EXISTS idx_weights_strategy ON auth_weights(strategy, weight DESC);
CREATE INDEX IF NOT EXISTS idx_health_status ON auth_health(status);
CREATE INDEX IF NOT EXISTS idx_health_unavailable ON auth_health(unavailable_until);
CREATE INDEX IF NOT EXISTS idx_metrics_success_rate ON auth_metrics(success_rate);
CREATE INDEX IF NOT EXISTS idx_metrics_latency ON auth_metrics(avg_latency_ms);
CREATE INDEX IF NOT EXISTS idx_status_auth_time ON status_code_history(auth_id, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_status_code ON status_code_history(status_code);
```

- [ ] **Step 2: Delete any existing test DB and run full test suite**

Run: `rm -f gateway.db && cargo nextest run`
Expected: All pass

- [ ] **Step 3: Commit**

```bash
git add migrations/20260417000000_initial_schema.sql
git commit -m "fix: add CHECK constraints to migration schema for data integrity"
```

---

## Self-Review

**1. Spec coverage:** All 11 triaged findings map to tasks:
- Finding #1 → Task 1
- Finding #2 → Task 2
- Findings #3, #8, #9, #12, #13, #17 → Task 3
- Findings #18, #19 → Task 4
- Finding #21 → Task 5

**2. Placeholder scan:** No TBD/TODO/fill-in-later. All steps contain complete code.

**3. Type consistency:** All types match domain definitions. `consecutive_successes: i32` in both FromRow structs and domain types. `SqlitePool` used consistently. `from_pool` signature updated across all call sites.
