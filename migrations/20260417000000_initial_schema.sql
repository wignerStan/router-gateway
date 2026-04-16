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
