# Single src Layout Refactor Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Consolidate the 6-crate workspace into a single Rust package with a standard `src/` layout.

**Architecture:** All workspace crates become modules under a single `src/` directory. The gateway package remains the root, with `gateway-utils` → `utils`, `llm-tracing` → `tracing`, `model-registry` → `registry`, `smart-routing` → `routing`. The CLI becomes a `[[bin]]` target. Import paths change from crate-level (`gateway_utils::...`) to module-level (`crate::utils::...`).

**Tech Stack:** Rust 1.85+, Tokio, Axum, SQLite (rusqlite) — no new dependencies.

---

## Current → Target Layout

```
BEFORE (workspace)                          AFTER (single src)
─────────────────────                       ─────────────────────
router-gateway/                             router-gateway/
├── Cargo.toml          (workspace)         ├── Cargo.toml          (single package)
├── cli/                                    ├── src/
│   ├── Cargo.toml                          │   ├── main.rs          (gateway binary)
│   ├── src/main.rs                         │   ├── lib.rs           (library root)
│   └── tests/cli_integration.rs            │   ├── config.rs
├── crates/                                 │   ├── routes.rs
│   ├── gateway/                            │   ├── state.rs
│   │   ├── Cargo.toml                      │   ├── providers/
│   │   ├── src/                            │   │   ├── mod.rs
│   │   │   ├── lib.rs                      │   │   ├── anthropic.rs
│   │   │   ├── main.rs                     │   │   ├── google.rs
│   │   │   ├── config.rs                   │   │   ├── openai.rs
│   │   │   ├── routes.rs                   │   │   └── types/
│   │   │   ├── state.rs                    │   │       ├── mod.rs
│   │   │   └── providers/                  │   │       └── messages.rs
│   │   │       ├── mod.rs                  │   ├── utils/
│   │   │       ├── anthropic.rs            │   │   ├── mod.rs
│   │   │       ├── google.rs              │   │   ├── env.rs
│   │   │       ├── openai.rs              │   │   ├── security.rs
│   │   │       └── types/                  │   │   └── ssrf.rs
│   │   │           ├── mod.rs              │   ├── tracing/
│   │   │           └── messages.rs         │   │   ├── mod.rs
│   │   └── tests/                          │   │   ├── collector.rs
│   │       ├── config.rs                   │   │   ├── metrics.rs
│   │       └── routes.rs                   │   │   ├── middleware.rs
│   ├── gateway-utils/                      │   │   └── trace.rs
│   │   ├── Cargo.toml                      │   ├── registry/
│   │   └── src/                            │   │   ├── mod.rs
│   │       ├── lib.rs                      │   │   ├── categories.rs
│   │       ├── env.rs                      │   │   ├── fetcher.rs
│   │       ├── security.rs                 │   │   ├── info.rs
│   │       └── ssrf.rs                     │   │   ├── policy/
│   ├── llm-tracing/                        │   │   │   ├── mod.rs
│   │   ├── Cargo.toml                      │   │   │   ├── matcher.rs
│   │   └── src/                            │   │   │   ├── matcher_tests.rs
│   │       ├── lib.rs                      │   │   │   ├── registry.rs
│   │       ├── collector.rs                │   │   │   ├── templates/
│   │       ├── metrics.rs                  │   │   │   │   └── mod.rs
│   │       ├── middleware.rs               │   │   │   ├── tests.rs
│   │       └── trace.rs                    │   │   │   └── types.rs
│   ├── model-registry/                     │   │   └── registry/
│   │   ├── Cargo.toml                      │   │       ├── mod.rs
│   │   └── src/                            │   │       ├── operations.rs
│   │       ├── lib.rs                      │   │       └── tests.rs
│   │       ├── categories.rs               │   └── routing/
│   │       ├── fetcher.rs                  │       ├── mod.rs
│   │       ├── info.rs                     │       ├── bandit/
│   │       └── policy/                     │       ├── candidate.rs
│   │           ├── mod.rs                  │       ├── classification/
│   │           ├── matcher.rs              │       ├── config/
│   │           ├── matcher_tests.rs        │       ├── executor.rs
│   │           ├── registry.rs             │       ├── fallback/
│   │           ├── templates/mod.rs        │       ├── filtering.rs
│   │           ├── tests.rs                │       ├── health/
│   │           └── types.rs                │       ├── history.rs
│   │       └── registry/                   │       ├── metrics.rs
│   │           ├── mod.rs                  │       ├── outcome.rs
│   │           ├── operations.rs           │       ├── policy_weight.rs
│   │           └── tests.rs                │       ├── reasoning.rs
│   └── smart-routing/                      │       ├── router/
│       ├── Cargo.toml                      │       ├── selector/
│       └── src/                            │       ├── session.rs
│           ├── lib.rs                      │       ├── sqlite/
│           ├── bandit/                     │       ├── statistics.rs
│           ├── candidate.rs                │       ├── utility.rs
│           ├── classification/             │       └── weight/
│           ├── config/                     └── bin/
│           ├── executor.rs                     └── cli.rs
│           ├── fallback/               ├── tests/
│           ├── filtering.rs                ├── config.rs
│           ├── health/                     ├── routes.rs
│           ├── history.rs                  ├── tracing_integration.rs
│           ├── metrics.rs                  ├── routing_integration.rs
│           ├── outcome.rs                  └── cli_integration.rs
│           ├── policy_weight.rs        ├── justfile
│           ├── reasoning.rs           ├── .github/workflows/ci.yml
│           ├── router/                 └── scripts/
│           ├── selector/
│           ├── session.rs
│           ├── sqlite/
│           ├── statistics.rs
│           ├── utility.rs
│           └── weight/
```

## Import Path Mapping

All files need these mechanical replacements:

| Old (workspace)                        | New (single crate)                          |
|----------------------------------------|---------------------------------------------|
| `gateway_utils::expand_env_var`        | `crate::utils::expand_env_var`              |
| `gateway_utils::constant_time_token_matches` | `crate::utils::constant_time_token_matches` |
| `gateway_utils::validate_url_not_private` | `crate::utils::validate_url_not_private`    |
| `llm_tracing::MemoryTraceCollector`    | `crate::tracing::MemoryTraceCollector`      |
| `llm_tracing::TracingMiddleware`       | `crate::tracing::TracingMiddleware`          |
| `llm_tracing::tracing_middleware`      | `crate::tracing::tracing_middleware`         |
| `llm_tracing::TraceCollector`          | `crate::tracing::TraceCollector`             |
| `llm_tracing::TraceMetrics`            | `crate::tracing::TraceMetrics`               |
| `llm_tracing::TraceSpan`               | `crate::tracing::TraceSpan`                  |
| `llm_tracing::TracingMiddlewareBuilder`| `crate::tracing::TracingMiddlewareBuilder`   |
| `model_registry::Registry`             | `crate::registry::Registry`                  |
| `model_registry::ModelInfo`            | `crate::registry::ModelInfo`                 |
| `model_registry::PolicyMatcher`        | `crate::registry::PolicyMatcher`             |
| `model_registry::PolicyRegistry`       | `crate::registry::PolicyRegistry`            |
| `model_registry::PolicyContext`        | `crate::registry::PolicyContext`             |
| `model_registry::DataSource`           | `crate::registry::DataSource`                |
| `model_registry::ModelCapabilities`    | `crate::registry::ModelCapabilities`         |
| `model_registry::RateLimits`           | `crate::registry::RateLimits`                |
| `smart_routing::` (any path)           | `crate::routing::` (same sub-path)           |

**In integration tests** (`tests/` directory), the crate name is the package name:
| Old                                  | New                                      |
|--------------------------------------|------------------------------------------|
| `gateway::config::GatewayConfig`     | `gateway::config::GatewayConfig`         |
| `gateway::build_app_state`           | `gateway::build_app_state`               |
| `llm_tracing::` (any path)           | `gateway::tracing::` (same sub-path)     |
| `model_registry::` (any path)        | `gateway::registry::` (same sub-path)    |
| `smart_routing::` (any path)         | `gateway::routing::` (same sub-path)     |

---

### Task 1: Create new Cargo.toml (single package)

**Files:**
- Replace: `Cargo.toml`

- [ ] **Step 1: Write the new single-package Cargo.toml**

Replace the workspace Cargo.toml with a single package. Merge all dependencies from all 6 crates. The package name stays `gateway`.

```toml
[package]
name = "gateway"
version = "0.1.0"
edition = "2024"
license = "MIT OR Apache-2.0"
description = "Local LLM gateway for intelligent request routing"
repository = "https://github.com/proxy/gateway"
rust-version = "1.85"

[lib]
path = "src/lib.rs"

[[bin]]
name = "gateway"
path = "src/main.rs"

[[bin]]
name = "gateway-cli"
path = "src/bin/cli.rs"

[dependencies]
# Async runtime
tokio = { version = "1.40", features = ["sync", "time", "rt", "rt-multi-thread", "macros"] }
tokio-util = "0.7"
tokio-stream = "0.1"
async-trait = "0.1"
futures = "0.3.32"

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
serde_yaml_ng = "0.10"

# Error handling
thiserror = "1.0"
anyhow = "1.0"

# HTTP/Web
axum = { version = "0.7", features = ["macros"] }
tower = { version = "0.5", features = ["util"] }
tower-http = { version = "0.5", features = ["trace", "cors", "set-header"] }
http = "1.0"
hyper = { version = "1.0", features = ["full"] }
reqwest = { version = "0.12", features = ["json"] }

# Observability
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# Time
chrono = { version = "0.4", features = ["serde"] }

# CLI
clap = { version = "4.4", features = ["derive"] }
colored = "2.1"
tabled = "0.15"

# Utilities
uuid = { version = "1.0", features = ["v4", "serde"] }
subtle = "2.6"
url = "2.5"
rand = "0.8"

# Database
rusqlite = { version = "0.31", features = ["bundled"] }

# Validation
jsonschema = "0.22"

[dev-dependencies]
assert_cmd = "2.0"
predicates = "3.1"
tempfile = "3"
wiremock = "0.6"
rstest = "0.25"
pretty_assertions = "1.4"
http-body-util = "0.1"

[lints.rust]
unsafe_code = "forbid"
missing_docs = "warn"

[lints.clippy]
all = { level = "deny", priority = -1 }
pedantic = { level = "warn", priority = -1 }
nursery = { level = "warn", priority = -1 }
perf = { level = "deny", priority = -1 }

# Production quality gates
collapsible_if = "deny"
uninlined_format_args = "deny"
redundant_closure_for_method_calls = "deny"
manual_range_contains = "deny"
unwrap_used = "deny"
expect_used = "deny"
missing_errors_doc = "deny"
missing_panics_doc = "deny"

# Casting: precision loss is acceptable for metric calculations, token counts, and costs
cast_possible_truncation = "allow"
cast_possible_wrap = "allow"
cast_precision_loss = "allow"
cast_sign_loss = "allow"
module_name_repetitions = "allow"
similar_names = "allow"
struct_excessive_bools = "allow"
```

- [ ] **Step 2: Commit**

```bash
git add Cargo.toml
git commit -m "refactor: convert workspace Cargo.toml to single package"
```

---

### Task 2: Create `src/lib.rs` (library root)

**Files:**
- Create: `src/lib.rs`

- [ ] **Step 1: Create the `src/` directory and write `src/lib.rs`**

This file declares all top-level modules. The crate-level re-exports from the old `gateway/src/lib.rs` (the `run()`, `load_config()`, `build_app_state()`, `build_app_router()` functions) move here from the old gateway lib.rs body.

```rust
//! Local LLM gateway with intelligent request routing.
//!
//! Routes LLM requests to optimal credentials based on health, latency,
//! and success rate. Designed for local development and self-hosted deployments.
//!
//! # Module layout
//!
//! - [`config`] — Gateway configuration loading and validation
//! - [`routes`] — HTTP route handlers and middleware
//! - [`state`] — Application state types (AppState, RateLimiter)
//! - [`providers`] — Provider adapters (OpenAI, Google, Anthropic)
//! - [`utils`] — Security utilities (timing-safe auth, SSRF protection)
//! - [`tracing`] — LLM request tracing and metrics
//! - [`registry`] — Model registry with multi-dimensional classification
//! - [`routing`] — Smart credential selection and routing

pub mod config;
pub mod providers;
pub mod routes;
pub mod state;
pub mod tracing;
pub mod registry;
pub mod routing;
pub mod utils;

use anyhow::Context;
use axum::{middleware, Router};
use crate::tracing::{MemoryTraceCollector, TracingMiddleware};
use crate::registry::Registry as ModelRegistry;
use crate::routing::{
    config::HealthConfig,
    executor::{ExecutorConfig, RouteExecutor},
    health::HealthManager,
    metrics::MetricsCollector,
    router::Router as SmartRouter,
    weight::AuthInfo,
};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use config::GatewayConfig;
use routes::{
    auth_middleware, chat_completions, health_check, list_models, rate_limit_middleware, root,
    route_request, security_headers_middleware,
};
use state::{AppState, DefaultRequestClassifier, RateLimiter, DEFAULT_RATE_LIMIT};

/// Build and run the gateway server.
///
/// # Errors
///
/// Returns an error if configuration loading fails, the configured
/// host/port is invalid, or the TCP listener cannot bind.
pub async fn run() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "gateway=debug,tower_http=debug,axum=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load configuration
    let config = load_config()?;
    tracing::info!(
        "Loaded configuration with {} credentials",
        config.credentials.len()
    );

    // Warn if authentication is disabled
    if !config.is_auth_enabled() {
        tracing::warn!(
            "WARNING: No auth_tokens configured — authentication is DISABLED. \
             This is not recommended for production deployments."
        );
    }

    let state = build_app_state(config, None);

    // Get host and port from config before moving state
    let port = state.config.server.port;
    let host = &state.config.server.host;
    let addr: SocketAddr = format!("{host}:{port}")
        .parse()
        .context("Invalid host/port configuration")?;
    tracing::info!("Gateway listening on {}", addr);

    // Periodically prune expired rate-limit buckets to bound memory growth.
    let prune_limiter = Arc::clone(&state.rate_limiter);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(120));
        loop {
            interval.tick().await;
            prune_limiter.prune();
        }
    });

    let app = build_app_router(state);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    Ok(())
}

/// Load configuration from file or environment.
///
/// # Errors
///
/// Returns an error if the configuration file exists but cannot be read
/// or parsed, or if validation fails.
pub fn load_config() -> anyhow::Result<GatewayConfig> {
    // Try to load from GATEWAY_CONFIG env var or default paths
    let config_path = std::env::var("GATEWAY_CONFIG").ok().or_else(|| {
        ["./gateway.yaml", "./config/gateway.yaml", "./gateway.yml"]
            .iter()
            .find(|path| std::path::Path::new(path).exists())
            .map(std::string::ToString::to_string)
    });

    if let Some(path) = config_path {
        tracing::info!("Loading configuration from {}", path);
        match GatewayConfig::from_file(&path) {
            Ok(config) => Ok(config),
            Err(e) => {
                anyhow::bail!(
                    "Failed to load config from {path}: {e}. Please fix the configuration file."
                );
            },
        }
    } else {
        tracing::info!("No configuration file found, using defaults");
        Ok(GatewayConfig::default())
    }
}

/// Creates the application state from the given config.
///
/// Shared by [`run()`] and test helpers to ensure production and test setups
/// stay in sync. The `rate_limit` parameter overrides the default when
/// provided.
#[must_use]
pub fn build_app_state(config: GatewayConfig, rate_limit: Option<u64>) -> AppState {
    let smart_router = config
        .credentials
        .iter()
        .fold(SmartRouter::new(), |mut router, cred| {
            router.add_credential(cred.id.clone(), cred.allowed_models.clone());
            router
        });

    let metrics = MetricsCollector::new();
    let health = HealthManager::new(HealthConfig::default());

    let executor = Arc::new(RouteExecutor::new(
        ExecutorConfig::default(),
        metrics,
        health,
    ));

    let classifier = Arc::new(DefaultRequestClassifier);

    let collector = Arc::new(MemoryTraceCollector::with_default_size());
    let tracing_middleware = TracingMiddleware::new(collector);

    let credentials: Vec<AuthInfo> = config
        .credentials
        .iter()
        .map(|c| AuthInfo {
            id: c.id.clone(),
            priority: Some(c.priority),
            quota_exceeded: false,
            unavailable: false,
            model_states: vec![],
        })
        .collect();

    let rate_limit = rate_limit.unwrap_or(DEFAULT_RATE_LIMIT);
    let rate_limiter = Arc::new(RateLimiter::new(rate_limit));

    AppState {
        config,
        registry: ModelRegistry::default(),
        router: smart_router,
        executor,
        classifier,
        tracing: tracing_middleware,
        start_time: Instant::now(),
        credentials,
        rate_limiter,
    }
}

/// Constructs the complete Axum router with all middleware layers in
/// production order.
///
/// Shared by [`run()`] and test helpers to guarantee the router structure
/// never diverges between production and test builds.
pub fn build_app_router(state: AppState) -> Router {
    let public_routes = Router::new()
        .route("/", axum::routing::get(root))
        .route("/health", axum::routing::get(health_check));

    let protected_routes = Router::new()
        .route("/api/models", axum::routing::get(list_models))
        .route("/api/route", axum::routing::get(route_request))
        .route(
            "/v1/chat/completions",
            axum::routing::post(chat_completions),
        )
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            rate_limit_middleware,
        ))
        .layer(middleware::from_fn(security_headers_middleware))
        .layer(middleware::from_fn_with_state(
            state.tracing.clone(),
            crate::tracing::tracing_middleware,
        ))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
```

---

### Task 3: Move gateway-utils → src/utils/

**Files:**
- Create: `src/utils/mod.rs` (from `crates/gateway-utils/src/lib.rs`)
- Move: `crates/gateway-utils/src/env.rs` → `src/utils/env.rs`
- Move: `crates/gateway-utils/src/security.rs` → `src/utils/security.rs`
- Move: `crates/gateway-utils/src/ssrf.rs` → `src/utils/ssrf.rs`

- [ ] **Step 1: Create `src/utils/mod.rs`**

Copy the body from `crates/gateway-utils/src/lib.rs`. The content stays identical — it declares the same sub-modules and re-exports.

```rust
//! Shared security and utility functions for the gateway.
//!
//! Provides timing-safe authentication, SSRF protection, and
//! environment variable expansion.

/// Environment variable expansion utilities.
pub mod env;
/// Timing-safe token comparison utilities.
pub mod security;
/// SSRF (Server-Side Request Forgery) protection.
pub mod ssrf;

pub use env::expand_env_var;
pub use security::constant_time_token_matches;
pub use ssrf::validate_url_not_private;
```

- [ ] **Step 2: Copy env.rs, security.rs, ssrf.rs verbatim**

No import changes needed — these files have no internal crate references.

```bash
cp crates/gateway-utils/src/env.rs src/utils/env.rs
cp crates/gateway-utils/src/security.rs src/utils/security.rs
cp crates/gateway-utils/src/ssrf.rs src/utils/ssrf.rs
```

---

### Task 4: Move llm-tracing → src/tracing/

**Files:**
- Create: `src/tracing/mod.rs` (from `crates/llm-tracing/src/lib.rs`)
- Move: `crates/llm-tracing/src/collector.rs` → `src/tracing/collector.rs`
- Move: `crates/llm-tracing/src/metrics.rs` → `src/tracing/metrics.rs`
- Move: `crates/llm-tracing/src/middleware.rs` → `src/tracing/middleware.rs`
- Move: `crates/llm-tracing/src/trace.rs` → `src/tracing/trace.rs`

- [ ] **Step 1: Create `src/tracing/mod.rs`**

```rust
//! Request/response observability for LLM requests.
//!
//! Captures trace spans with timing, token usage, and error information.
//! Integrates with Axum as middleware and aggregates metrics per model
//! and provider for monitoring and cost analysis.

/// In-memory trace collection with bounded buffering.
pub mod collector;
/// Metrics aggregation for traces, per-provider and per-model.
pub mod metrics;
/// Axum middleware for automatic request tracing.
pub mod middleware;
/// Trace span data types for LLM requests.
pub mod trace;

pub use collector::{MemoryTraceCollector, TraceCollector};
pub use metrics::{ModelMetrics, ProviderMetrics, TraceMetrics};
pub use middleware::{tracing_middleware, TracingMiddleware, TracingMiddlewareBuilder};
pub use trace::TraceSpan;

/// Errors produced during trace collection and processing.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A trace could not be collected.
    #[error("trace collection error: {0}")]
    Collection(String),
}

/// Result type for fallible operations in this module.
pub type Result<T> = std::result::Result<T, Error>;
```

- [ ] **Step 2: Copy collector.rs, metrics.rs, middleware.rs, trace.rs verbatim**

No import changes needed — these files only use external crates (tokio, chrono, serde, etc.), not internal workspace crates.

```bash
cp crates/llm-tracing/src/collector.rs src/tracing/collector.rs
cp crates/llm-tracing/src/metrics.rs src/tracing/metrics.rs
cp crates/llm-tracing/src/middleware.rs src/tracing/middleware.rs
cp crates/llm-tracing/src/trace.rs src/tracing/trace.rs
```

---

### Task 5: Move model-registry → src/registry/

**Files:**
- Create: `src/registry/mod.rs` (from `crates/model-registry/src/lib.rs`)
- Move: `crates/model-registry/src/categories.rs` → `src/registry/categories.rs`
- Move: `crates/model-registry/src/fetcher.rs` → `src/registry/fetcher.rs`
- Move: `crates/model-registry/src/info.rs` → `src/registry/info.rs`
- Move: `crates/model-registry/src/policy/` → `src/registry/policy/`
- Move: `crates/model-registry/src/registry/` → `src/registry/registry/`

- [ ] **Step 1: Create `src/registry/mod.rs`**

```rust
//! Model metadata registry with multi-dimension categorization and routing policy.
//!
//! Provides model discovery, capability classification, and policy-based routing
//! rules. Used by the gateway to select optimal credentials based on model
//! characteristics, provider constraints, and cost preferences.

/// Multi-dimensional classification for model routing.
pub mod categories;
/// Model data fetching interface and static implementation.
pub mod fetcher;
/// Model metadata types, capabilities, and validation.
pub mod info;
/// Multi-dimensional routing policy configuration.
pub mod policy;
/// Thread-safe model registry with caching and coalesced fetches.
pub mod registry;

pub use categories::{
    CapabilityCategory, ContextWindowCategory, CostCategory, ModelCategorization, ProviderCategory,
    TierCategory,
};
pub use fetcher::{ModelFetcher, StaticFetcher};
pub use info::{DataSource, ModelCapabilities, ModelInfo, RateLimits};
pub use policy::templates;
pub use policy::{
    ModalityCategory, PolicyAction, PolicyCondition, PolicyConditionType, PolicyContext,
    PolicyFilters, PolicyLoadError, PolicyMatch, PolicyMatcher, PolicyRegistry, RoutingPolicy,
};
pub use registry::Registry;

/// Top-level error type for the model registry module.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The requested model was not found.
    #[error("model not found: {0}")]
    ModelNotFound(String),
    /// A policy operation failed.
    #[error("policy error: {0}")]
    Policy(String),
    /// The model ID could not be parsed.
    #[error("cannot parse model ID: {0}")]
    InvalidModelId(String),
}

/// Module-level [`Result`] alias.
pub type Result<T> = std::result::Result<T, Error>;
```

- [ ] **Step 2: Copy all source files**

```bash
cp crates/model-registry/src/categories.rs src/registry/categories.rs
cp crates/model-registry/src/fetcher.rs src/registry/fetcher.rs
cp crates/model-registry/src/info.rs src/registry/info.rs
cp -r crates/model-registry/src/policy src/registry/policy
cp -r crates/model-registry/src/registry src/registry/registry
```

No import changes needed — model-registry has no internal workspace dependencies.

---

### Task 6: Move smart-routing → src/routing/

**Files:**
- Create: `src/routing/mod.rs` (from `crates/smart-routing/src/lib.rs`)
- Move entire `crates/smart-routing/src/` tree → `src/routing/`

This is the largest module and the only one with internal workspace deps (`model_registry`).

- [ ] **Step 1: Create `src/routing/mod.rs`**

```rust
//! Intelligent credential selection for LLM request routing.
//!
//! Provides health-aware, latency-optimized routing based on success rates,
//! configurable weight factors, and multiple selection strategies including
//! weighted random, time-aware, quota-aware, and adaptive routing.

/// Multi-armed bandit exploration strategies.
pub mod bandit;
/// Route candidate evaluation and scoring.
pub mod candidate;
/// Request classification for routing decisions.
pub mod classification;
/// Routing configuration types.
pub mod config;
/// Route execution and result handling.
pub mod executor;
/// Fallback planning and route recovery.
pub mod fallback;
/// Constraint filtering for candidate selection.
pub mod filtering;
/// Credential health tracking and status management.
pub mod health;
/// Attempt history and decision tracking.
pub mod history;
/// Credential metrics collection and aggregation.
pub mod metrics;
/// Execution outcome recording.
pub mod outcome;
/// Policy-aware weight calculation.
pub mod policy_weight;
/// Reasoning capability inference.
pub mod reasoning;
/// Request routing and dispatch.
pub mod router;
/// Smart credential selector.
pub mod selector;
/// Session affinity management.
pub mod session;
/// SQLite-backed persistence for metrics and health.
pub mod sqlite;
/// Route statistics aggregation and time-bucket analysis.
pub mod statistics;
/// Utility estimation for route selection.
pub mod utility;
/// Credential weight calculation strategies.
pub mod weight;

pub use bandit::{BanditConfig, BanditPolicy, RouteStats};
pub use candidate::{
    check_capability_support, CandidateBuilder, CapabilitySupport, RouteCandidate, TokenFitStatus,
};
pub use classification::{
    ClassifiedRequest, FormatDetector, QualityPreference, RequestClassifier, RequestFormat,
    RequiredCapabilities, TokenEstimator,
};
pub use config::{
    HealthConfig, PolicyConfig, QuotaAwareConfig, SmartRoutingConfig, TimeAwareConfig, WeightConfig,
};
pub use executor::{ExecutionResult, ExecutorConfig, RouteExecutor};
pub use fallback::{FallbackConfig, FallbackPlanner, FallbackRoute};
pub use filtering::{ConstraintFilter, FilterResult};
pub use health::{AuthHealth, HealthManager, HealthStatus};
pub use history::{
    AttemptHistory, AttemptMetrics, DecisionContext, RouteAttempt, SelectionMode, TrackingSystem,
};
pub use metrics::{AuthMetrics, MetricsCollector};
pub use outcome::{ErrorClass, ExecutionOutcome, OutcomeRecorder};
pub use policy_weight::{
    PolicyAwareWeightCalculator, PolicyWeightCalculator, WeightCalculatorFactory,
};
pub use reasoning::{ReasoningCapability, ReasoningInference, ReasoningRequest};
pub use router::Router;
pub use selector::SmartSelector;
pub use session::{SessionAffinity, SessionAffinityManager, SessionStats};
pub use sqlite::{
    SQLiteConfig, SQLiteHealthManager, SQLiteMetricsCollector, SQLiteSelector, SQLiteStore,
    SelectorStats,
};
pub use statistics::{
    BucketStatistics, ColdStartPriors, RouteStatistics, StatisticsAggregator, TimeBucket,
};
pub use utility::{UtilityConfig, UtilityEstimator};
pub use weight::{AuthInfo, DefaultWeightCalculator, ModelState, WeightCalculator};

pub use sqlite::error::SqliteError;

/// Top-level error type for the routing module.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Error from the `SQLite` backend via [`SqliteError`].
    #[error(transparent)]
    Sqlite(#[from] SqliteError),
    /// Configuration error.
    #[error("configuration error: {0}")]
    Config(String),
    /// No candidates available for routing.
    #[error("no candidates available for routing")]
    NoCandidates,
}

/// Result type for the routing module.
pub type Result<T> = std::result::Result<T, Error>;
```

- [ ] **Step 2: Copy entire smart-routing source tree**

```bash
# Copy all individual files
cp crates/smart-routing/src/candidate.rs src/routing/candidate.rs
cp crates/smart-routing/src/executor.rs src/routing/executor.rs
cp crates/smart-routing/src/filtering.rs src/routing/filtering.rs
cp crates/smart-routing/src/history.rs src/routing/history.rs
cp crates/smart-routing/src/metrics.rs src/routing/metrics.rs
cp crates/smart-routing/src/outcome.rs src/routing/outcome.rs
cp crates/smart-routing/src/policy_weight.rs src/routing/policy_weight.rs
cp crates/smart-routing/src/reasoning.rs src/routing/reasoning.rs
cp crates/smart-routing/src/session.rs src/routing/session.rs
cp crates/smart-routing/src/statistics.rs src/routing/statistics.rs
cp crates/smart-routing/src/utility.rs src/routing/utility.rs

# Copy subdirectory modules
cp -r crates/smart-routing/src/bandit src/routing/bandit
cp -r crates/smart-routing/src/classification src/routing/classification
cp -r crates/smart-routing/src/config src/routing/config
cp -r crates/smart-routing/src/fallback src/routing/fallback
cp -r crates/smart-routing/src/health src/routing/health
cp -r crates/smart-routing/src/router src/routing/router
cp -r crates/smart-routing/src/selector src/routing/selector
cp -r crates/smart-routing/src/sqlite src/routing/sqlite
cp -r crates/smart-routing/src/weight src/routing/weight
```

- [ ] **Step 3: Replace `model_registry::` with `crate::registry::` in all routing files**

This is a mechanical find-and-replace across the routing module. Files affected:

- `src/routing/candidate.rs` (lines 8, 215)
- `src/routing/filtering.rs` (lines 9, 189)
- `src/routing/policy_weight.rs` (lines 13, 165)
- `src/routing/selector/mod.rs` (line 13)
- `src/routing/selector/ranking.rs` (line 4)
- `src/routing/selector/tests.rs` (line 201)
- `src/routing/router/dispatch.rs` (line 10)
- `src/routing/router/tests.rs` (line 10)

Run:

```bash
find src/routing -name "*.rs" -exec sed -i 's/use model_registry::/use crate::registry::/g' {} +
```

Verify each changed file still compiles by checking the imports are correct. For example, `src/routing/candidate.rs`:

```rust
// BEFORE:
use model_registry::ModelInfo;

// AFTER:
use crate::registry::ModelInfo;
```

---

### Task 7: Move gateway core modules → src/

**Files:**
- Create: `src/main.rs` (from `crates/gateway/src/main.rs`)
- Move: `crates/gateway/src/config.rs` → `src/config.rs`
- Move: `crates/gateway/src/routes.rs` → `src/routes.rs`
- Move: `crates/gateway/src/state.rs` → `src/state.rs`
- Move: `crates/gateway/src/providers/` → `src/providers/`

- [ ] **Step 1: Create `src/main.rs`**

```rust
//! Gateway binary entrypoint.

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    gateway::run().await
}
```

- [ ] **Step 2: Copy config.rs, routes.rs, state.rs**

```bash
cp crates/gateway/src/config.rs src/config.rs
cp crates/gateway/src/routes.rs src/routes.rs
cp crates/gateway/src/state.rs src/state.rs
cp -r crates/gateway/src/providers src/providers
```

- [ ] **Step 3: Update imports in src/config.rs**

```rust
// BEFORE (line 7-8):
use gateway_utils::{expand_env_var, validate_url_not_private};

// AFTER:
use crate::utils::{expand_env_var, validate_url_not_private};
```

```rust
// BEFORE (line 12):
pub use gateway_utils::constant_time_token_matches;

// AFTER:
pub use crate::utils::constant_time_token_matches;
```

- [ ] **Step 4: Update imports in src/state.rs**

```rust
// BEFORE (lines 9-18):
use llm_tracing::TracingMiddleware;
use model_registry::Registry as ModelRegistry;
use smart_routing::executor::RouteExecutor;
use smart_routing::{
    classification::{
        ContentTypeDetector, FormatDetector, RequestClassifier, StreamingExtractor, ToolDetector,
    },
    router::Router as SmartRouter,
    weight::AuthInfo,
};

// AFTER:
use crate::tracing::TracingMiddleware;
use crate::registry::Registry as ModelRegistry;
use crate::routing::executor::RouteExecutor;
use crate::routing::{
    classification::{
        ContentTypeDetector, FormatDetector, RequestClassifier, StreamingExtractor, ToolDetector,
    },
    router::Router as SmartRouter,
    weight::AuthInfo,
};
```

```rust
// BEFORE (line 69):
use smart_routing::classification::{ClassifiedRequest, QualityPreference, RequiredCapabilities};

// AFTER:
use crate::routing::classification::{ClassifiedRequest, QualityPreference, RequiredCapabilities};
```

- [ ] **Step 5: Update imports in src/routes.rs**

```rust
// BEFORE (line 9):
use smart_routing::classification::RequestClassifier;

// AFTER:
use crate::routing::classification::RequestClassifier;
```

- [ ] **Step 6: Verify providers/ needs no changes**

The providers module only uses `crate::` local references and external crates. Check that `src/providers/mod.rs` and all provider files don't reference workspace crates. They should not — providers only use `serde`, `serde_json`, `serde_yaml_ng`, and local `crate::providers::types` references.

---

### Task 8: Move CLI → src/bin/cli.rs

**Files:**
- Create: `src/bin/cli.rs` (from `cli/src/main.rs`)

- [ ] **Step 1: Create `src/bin/` directory and copy CLI**

```bash
mkdir -p src/bin
cp cli/src/main.rs src/bin/cli.rs
```

No import changes needed — the CLI has no internal workspace dependencies. The file uses only external crates (`clap`, `reqwest`, `serde`, `colored`, `tabled`, etc.) which are now dependencies of the single package.

The binary name will be `gateway-cli` (as declared in Cargo.toml `[[bin]]`).

---

### Task 9: Move and update integration tests

**Files:**
- Move: `crates/gateway/tests/config.rs` → `tests/config.rs`
- Move: `crates/gateway/tests/routes.rs` → `tests/routes.rs`
- Move: `crates/llm-tracing/tests/integration_test.rs` → `tests/tracing_integration.rs`
- Move: `crates/smart-routing/tests/bdd_integration_tests.rs` → `tests/routing_integration.rs`
- Move: `cli/tests/cli_integration.rs` → `tests/cli_integration.rs`

- [ ] **Step 1: Create tests/ directory and copy files**

```bash
mkdir -p tests
cp crates/gateway/tests/config.rs tests/config.rs
cp crates/gateway/tests/routes.rs tests/routes.rs
cp crates/llm-tracing/tests/integration_test.rs tests/tracing_integration.rs
cp crates/smart-routing/tests/bdd_integration_tests.rs tests/routing_integration.rs
cp cli/tests/cli_integration.rs tests/cli_integration.rs
```

- [ ] **Step 2: Update imports in tests/config.rs**

```rust
// BEFORE (line 3):
use gateway::config::GatewayConfig;

// AFTER (stays the same — package name is "gateway"):
use gateway::config::GatewayConfig;
```

No changes needed — `gateway::config::GatewayConfig` works because the package name is `gateway`.

- [ ] **Step 3: Update imports in tests/routes.rs**

```rust
// BEFORE (lines 14-34):
use gateway::config::{CredentialConfig, GatewayConfig};
use gateway::routes::{
    health_check, list_models, rate_limit_middleware, root, route_request,
    security_headers_middleware,
};
use gateway::state::{AppState, DefaultRequestClassifier, HealthStatus, RateLimiter};
use gateway::{build_app_router, build_app_state};
use llm_tracing::{MemoryTraceCollector, TracingMiddleware};
use model_registry::{
    DataSource, ModelCapabilities, ModelInfo as RegistryModelInfo, RateLimits,
    Registry as ModelRegistry,
};
use smart_routing::classification::RequestClassifier;
use smart_routing::{
    config::HealthConfig,
    executor::{ExecutorConfig, RouteExecutor},
    health::HealthManager,
    metrics::MetricsCollector,
    router::Router as SmartRouter,
};

// AFTER:
use gateway::config::{CredentialConfig, GatewayConfig};
use gateway::routes::{
    health_check, list_models, rate_limit_middleware, root, route_request,
    security_headers_middleware,
};
use gateway::state::{AppState, DefaultRequestClassifier, HealthStatus, RateLimiter};
use gateway::{build_app_router, build_app_state};
use gateway::tracing::{MemoryTraceCollector, TracingMiddleware};
use gateway::registry::{
    DataSource, ModelCapabilities, ModelInfo as RegistryModelInfo, RateLimits,
    Registry as ModelRegistry,
};
use gateway::routing::classification::RequestClassifier;
use gateway::routing::{
    config::HealthConfig,
    executor::{ExecutorConfig, RouteExecutor},
    health::HealthManager,
    metrics::MetricsCollector,
    router::Router as SmartRouter,
};
```

- [ ] **Step 4: Update imports in tests/tracing_integration.rs**

```rust
// BEFORE (lines 14-17):
use llm_tracing::{
    MemoryTraceCollector, TraceCollector, TraceMetrics, TraceSpan, TracingMiddleware,
    TracingMiddlewareBuilder,
};

// AFTER:
use gateway::tracing::{
    MemoryTraceCollector, TraceCollector, TraceMetrics, TraceSpan, TracingMiddleware,
    TracingMiddlewareBuilder,
};
```

- [ ] **Step 5: Update imports in tests/routing_integration.rs**

```rust
// BEFORE (line 12):
use smart_routing::classification::ContentTypeDetector;

// AFTER:
use gateway::routing::classification::ContentTypeDetector;
```

Scan the entire file for any remaining `smart_routing::` or `model_registry::` references and replace:
- `smart_routing::` → `gateway::routing::`
- `model_registry::` → `gateway::registry::`

- [ ] **Step 6: Update tests/cli_integration.rs**

The CLI integration test uses `assert_cmd` to run the binary. The binary name changes from `cli` to `gateway-cli`:

```rust
// Find any reference to the binary name "cli" and change to "gateway-cli"
```

Search for `crate_name!("cli")` or similar and update to `crate_name!("gateway-cli")`.

---

### Task 10: Delete old workspace directories

**Files:**
- Delete: `cli/`
- Delete: `crates/`

- [ ] **Step 1: Verify the new src/ compiles first**

```bash
cargo check
```

Expected: Clean compilation (warnings OK, no errors).

If there are compilation errors, fix them before proceeding. Common issues:
- Missed import path changes
- Module not found errors
- Visibility issues (items that were `pub` in a crate but need `pub(crate)` now)

- [ ] **Step 2: Run all tests**

```bash
cargo test
```

Expected: All tests pass.

- [ ] **Step 3: Remove old workspace directories**

```bash
rm -rf cli/ crates/
```

- [ ] **Step 4: Verify everything still works after removal**

```bash
cargo check && cargo test
```

- [ ] **Step 5: Commit the refactor**

```bash
git add -A
git commit -m "refactor: consolidate workspace into single src layout

Merges 6 workspace crates (gateway, gateway-utils, llm-tracing,
model-registry, smart-routing, cli) into a single package with
modules under src/. Each former crate becomes a top-level module:
- gateway-utils → src/utils/
- llm-tracing → src/tracing/
- model-registry → src/registry/
- smart-routing → src/routing/
- gateway core → src/{config,routes,state,providers}
- cli → src/bin/cli.rs

All internal imports changed from crate-level paths to module-level
paths (e.g., gateway_utils:: → crate::utils::)."
```

---

### Task 11: Update tooling and documentation

**Files:**
- Modify: `justfile`
- Modify: `.github/workflows/ci.yml`
- Modify: `.cargo/config.toml`
- Modify: `AGENTS.md`
- Modify: `scripts/check-lint-inheritance.sh`

- [ ] **Step 1: Update justfile**

Replace workspace-specific commands with single-package equivalents:

```diff
- members := "cli crates/gateway crates/smart-routing crates/model-registry crates/llm-tracing"
```

```diff
- # Run smart-routing tests
- test-routing:
-     cargo test -p smart-routing
-
- # Run model-registry tests
- test-registry:
-     cargo test -p model-registry
-
- # Run tracing tests
- test-tracing:
-     cargo test -p llm-tracing
-
- # Run gateway tests
- test-gateway:
-     cargo test -p gateway
+ # Run routing module tests
+ test-routing:
+     cargo test --lib routing
+
+ # Run registry module tests
+ test-registry:
+     cargo test --lib registry
+
+ # Run tracing module tests
+ test-tracing:
+     cargo test --lib tracing
+
+ # Run gateway core tests
+ test-gateway:
+     cargo test --lib -- --skip routing --skip registry --skip tracing --skip utils
```

```diff
- # Build specific package
- build-package PACKAGE:
-     cargo build -p {{PACKAGE}}
+ # Build specific binary
+ build-bin BIN:
+     cargo build --bin {{BIN}}
```

```diff
- # Run CLI tool
- cli *ARGS:
-     cargo run --bin cli -- {{ARGS}}
+ # Run CLI tool
-     cli *ARGS:
-     cargo run --bin gateway-cli -- {{ARGS}}
```

```diff
- # Run tests for specific package
- test-package PACKAGE:
-     cargo test -p {{PACKAGE}}
+ # Run tests matching pattern
+ test-package PATTERN:
+     cargo test {{PATTERN}}
```

```diff
  # Run the gateway app
  start:
      cargo run --bin gateway
```

```diff
- # Show workspace structure
- structure:
-     @echo "📁 Workspace Structure:"
-     @find crates cli -name "Cargo.toml" 2>/dev/null | head -20
+ # Show source structure
+ structure:
+     @echo "📁 Source Structure:"
+     @find src -name "*.rs" | head -40
```

```diff
- # Show binary sizes
- binary-sizes:
-     @echo "📊 Binary Sizes:"
-     @ls -lh target/debug/{gateway,my-cli} 2>/dev/null || echo "Build first with: just build"
+ # Show binary sizes
+ binary-sizes:
+     @echo "📊 Binary Sizes:"
+     @ls -lh target/debug/{gateway,gateway-cli} 2>/dev/null || echo "Build first with: just build"
```

Update coverage command to remove workspace-specific exclude paths:

```diff
-     cargo tarpaulin --all --all-features --out Xml --output-dir coverage --timeout 300 --exclude-files "cli/src/main.rs" --exclude-files "crates/gateway/src/main.rs"
+     cargo tarpaulin --all-features --out Xml --output-dir coverage --timeout 300 --exclude-files "src/main.rs" --exclude-files "src/bin/cli.rs"
```

- [ ] **Step 2: Update CI (.github/workflows/ci.yml)**

Remove workspace-specific flags and update binary artifact names:

```diff
-       - name: Run tests
-         run: cargo test --all --all-features
+       - name: Run tests
+         run: cargo test --all-features
```

```diff
-       - name: Generate coverage report
-         run: cargo tarpaulin --all --all-features --out Xml --output-dir coverage --timeout 300 --exclude-files "cli/src/main.rs" --exclude-files "crates/gateway/src/main.rs"
+       - name: Generate coverage report
+         run: cargo tarpaulin --all-features --out Xml --output-dir coverage --timeout 300 --exclude-files "src/main.rs" --exclude-files "src/bin/cli.rs"
```

```diff
-         run: cargo build --release --target ${{ matrix.target }}
+         run: cargo build --release --target ${{ matrix.target }}
```

```diff
-           cp target/${{ matrix.target }}/release/my-cli artifacts/${{ matrix.artifact }}-cli
+           cp target/${{ matrix.target }}/release/gateway-cli artifacts/${{ matrix.artifact }}-cli
```

Update Windows artifact preparation similarly:
```diff
-           copy target\${{ matrix.target }}\release\my-cli.exe artifacts\${{ matrix.artifact }}-cli
+           copy target\${{ matrix.target }}\release\gateway-cli.exe artifacts\${{ matrix.artifact }}-cli
```

- [ ] **Step 3: Update .cargo/config.toml**

```diff
- lint = "clippy --workspace --all-targets -- -D warnings"
+ lint = "clippy --all-targets -- -D warnings"
```

- [ ] **Step 4: Update scripts/check-lint-inheritance.sh**

This script checks that workspace members inherit workspace lints. Since we now have a single package, the lints are directly in Cargo.toml. Update the script to verify the single Cargo.toml has the expected lint sections, or simplify it to just check that `[lints]` exists.

- [ ] **Step 5: Update AGENTS.md**

Update all references to the old layout:

- Remove workspace-specific build commands (`cargo build -p gateway`, `cargo test -p smart-routing`, etc.)
- Remove `crates/gateway-utils`, `crates/smart-routing`, etc. references
- Update "Build and test commands" table to remove `-p` package flags
- Update "Known Pitfalls" to reflect single-crate module paths
- Update the module list and dependency descriptions

Key changes:

```diff
- **Stack**: Rust 1.85+, Tokio, Axum, SQLite
+ **Stack**: Rust 1.85+, Tokio, Axum, SQLite (single-crate layout)

- ### Package Names
- Directory names match Cargo package names throughout the workspace.

  ### Build and test commands
-
- | Task        | Command                  |
- | ----------- | ------------------------ |
- | Build       | `cargo build`            |
- | Test        | `cargo test --workspace` |
- | Run Gateway | `cargo run -p gateway`   |
- | Lint        | `cargo lint`             |
- | Format      | `cargo fmt`              |
- | All Checks  | `just qa`                |
+
+ | Task        | Command              |
+ | ----------- | -------------------- |
+ | Build       | `cargo build`        |
+ | Test        | `cargo test`         |
+ | Run Gateway | `cargo run --bin gateway` |
+ | Run CLI     | `cargo run --bin gateway-cli` |
+ | Lint        | `cargo lint`         |
+ | Format      | `cargo fmt`          |
+ | All Checks  | `just qa`            |
```

Remove the Package Names section. Update Known Pitfalls to replace crate-level paths with module paths.

- [ ] **Step 6: Remove per-crate CLAUDE.md and AGENTS.md files**

Delete `cli/CLAUDE.md`, `cli/AGENTS.md`, `crates/*/CLAUDE.md`, `crates/*/AGENTS.md` (already removed with the directories in Task 10).

- [ ] **Step 7: Commit tooling updates**

```bash
git add justfile .github/workflows/ci.yml .cargo/config.toml scripts/check-lint-inheritance.sh AGENTS.md
git commit -m "refactor: update tooling and docs for single src layout"
```

---

### Task 12: Final verification

- [ ] **Step 1: Run full QA pipeline**

```bash
just qa-full
```

Expected: All checks pass (fmt, check, clippy, tests, security audit).

- [ ] **Step 2: Verify binary builds**

```bash
cargo build --release
ls -lh target/release/gateway target/release/gateway-cli
```

Expected: Both binaries exist.

- [ ] **Step 3: Verify test coverage**

```bash
cargo tarpaulin --all-features --out Xml --output-dir coverage --timeout 300 --exclude-files "src/main.rs" --exclude-files "src/bin/cli.rs"
```

Expected: Coverage >= 80%.

---

## Self-Review

### Spec Coverage
- Workspace → single package: Covered in Tasks 1-8
- Import path changes: Covered in Tasks 6, 7, 9
- Test migration: Covered in Task 9
- Tooling updates: Covered in Task 11
- Cleanup: Covered in Task 10
- Verification: Covered in Task 12

### Placeholder Scan
No TBDs, TODOs, or "implement later" patterns found. All file paths, code blocks, and commands are explicit.

### Type Consistency
- `ModelInfo` alias in tests remains `RegistryModelInfo` (from `gateway::registry::ModelInfo`)
- `Router` alias remains `SmartRouter` (from `gateway::routing::Router`)
- `Registry` alias remains `ModelRegistry` (from `gateway::registry::Registry`)
- Error types stay module-scoped (`tracing::Error`, `registry::Error`, `routing::Error`)
