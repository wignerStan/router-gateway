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
