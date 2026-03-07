# Gateway API

A smart routing gateway for LLM (Large Language Model) requests built with Rust, Axum, and Tokio.

## Overview

The Gateway API provides intelligent routing, model registry management, and comprehensive tracing for LLM applications. It acts as a central entry point for routing requests to appropriate language models based on various criteria including cost, performance, and capabilities.

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         Gateway API                              │
│                                                                   │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐      │
│  │ Smart        │  │ Model        │  │ LLM              │      │
│  │ Routing      │  │ Registry     │  │ Tracing          │      │
│  │              │  │              │  │                  │      │
│  │ - Weight     │  │ - Cache      │  │ - Spans          │      │
│  │ - Health     │  │ - Fetcher    │  │ - Metrics        │      │
│  │ - Selector   │  │ - Categories │  │ - Middleware     │      │
│  └──────────────┘  └──────────────┘  └──────────────────┘      │
│                                                                   │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │              HTTP Layer (Axum)                            │   │
│  │  /health   /api/models   /api/route   /                  │   │
│  └──────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
                    ┌─────────────────┐
                    │  External LLMs  │
                    │  (OpenAI, etc.) │
                    └─────────────────┘
```

## Features

### Smart Routing
Intelligent request routing based on:
- **Weight-based selection**: Calculate optimal routes using configurable weights
- **Health monitoring**: Track service health and availability
- **Time-aware routing**: Consider request timing patterns
- **Quota management**: Respect rate limits and quotas
- **SQLite backend**: Persistent storage for metrics and health data

### Model Registry
Centralized model information management:
- **Caching**: TTL-based caching with background refresh
- **Dynamic fetching**: Pluggable fetcher architecture
- **Categorization**: Organize models by capabilities, tier, cost, and context window
- **Type-safe**: Strongly typed model information

### LLM Tracing
Comprehensive request tracing and monitoring:
- **Span tracking**: Distributed tracing for all LLM requests
- **Metrics collection**: Performance and usage metrics
- **Middleware integration**: Easy integration with Axum
- **In-memory collection**: Built-in trace collector for development

## Project Structure

```
gateway/
├── apps/
│   ├── gateway/          # Main gateway application
│   └── cli/              # CLI tool (optional)
├── packages/
│   ├── smart-routing/    # Smart routing logic
│   ├── model-registry/   # Model registry and caching
│   ├── tracing/          # LLM request tracing
│   └── core/             # Shared utilities
└── Cargo.toml            # Workspace configuration
```

## Getting Started

### Prerequisites

- Rust 1.70 or later
- Cargo (comes with Rust)

### Building

```bash
# Build the entire workspace
cargo build --workspace

# Build specific package
cargo build --package gateway
```

### Running

```bash
# Run the gateway server
cargo run --package gateway

# The server will start on http://localhost:3000
```

### Environment Variables

```bash
# Set log level (default: debug)
export RUST_LOG="gateway=debug,tower_http=debug,axum=debug"

# Run with custom log level
RUST_LOG="info" cargo run --package gateway
```

## API Endpoints

### GET `/health`
Health check endpoint.

**Response:**
```
OK
```

### GET `/`
API information endpoint.

**Response:**
```json
{
  "name": "Gateway API",
  "version": "0.1.0",
  "description": "Smart routing gateway for LLM requests",
  "features": [
    "Smart Routing",
    "Model Registry",
    "LLM Tracing"
  ]
}
```

### GET `/api/models`
List available models.

**Response:**
```json
{
  "models": [],
  "count": 0,
  "message": "Model registry integration pending"
}
```

### GET `/api/route`
Example routing endpoint.

**Response:**
```json
{
  "routed_to": "example-model",
  "status": "success",
  "message": "Smart routing integration pending"
}
```

## Usage Examples

### Basic Usage

```bash
# Start the gateway
cargo run --package gateway

# Test health endpoint
curl http://localhost:3000/health

# Test root endpoint
curl http://localhost:3000/

# Test models endpoint
curl http://localhost:3000/api/models

# Test routing endpoint
curl http://localhost:3000/api/route
```

### Integration Testing

The gateway includes integration tests that can be run with:

```bash
# Run all tests
cargo test --workspace

# Run gateway tests only
cargo test --package gateway

# Run with output
cargo test --package gateway -- --nocapture
```

## Configuration

### Smart Routing Configuration

```rust
use smart_routing::SmartRoutingConfig;

let config = SmartRoutingConfig {
    weight: WeightConfig::default(),
    health: HealthConfig::default(),
    time_aware: TimeAwareConfig::default(),
    quota_aware: QuotaAwareConfig::default(),
};
```

### SQLite Backend Configuration

The smart routing package includes a SQLite backend for persistent storage of metrics and health data.

#### Basic Setup

```rust
use smart_routing::sqlite::{SQLiteStore, SQLiteConfig};
use smart_routing::sqlite::{SQLiteMetricsCollector, SQLiteHealthManager, SQLiteSelector};

// Create SQLite store with in-memory database
let config = SQLiteConfig {
    database_path: ":memory:".to_string(),
    ..Default::default()
};

let store = SQLiteStore::new(config).await?;

// Or use file-based storage
let config = SQLiteConfig {
    database_path: "./routing.db".to_string(),
    ..Default::default()
};

let store = SQLiteStore::new(config).await?;
```

#### Configuration Options

```rust
use smart_routing::sqlite::SQLiteConfig;

let config = SQLiteConfig {
    database_path: "./routing.db".to_string(),
    max_history_entries: 10000,      // Maximum history entries to retain
    history_ttl_days: 7,              // History retention period in days
    connection_pool_size: 5,          // Number of connections in pool
};
```

#### Metrics Collection

```rust
use smart_routing::sqlite::SQLiteMetricsCollector;

let collector = SQLiteMetricsCollector::new(store);

// Initialize an auth service
collector.initialize_auth("my-auth-service").await;

// Record requests
collector
    .record_request("my-auth-service", 150.0, true, 200)
    .await;

// Get metrics
let metrics = collector.get_metrics("my-auth-service").await;
if let Some(metrics) = metrics {
    println!("Success rate: {}", metrics.success_rate);
    println!("Average latency: {}ms", metrics.avg_latency_ms);
}
```

#### Health Management

```rust
use smart_routing::sqlite::SQLiteHealthManager;

let manager = SQLiteHealthManager::new(store);

// Record successful request
manager.record_success("my-auth-service").await;

// Record failed request
manager.record_failure("my-auth-service", 500).await;

// Check health status
let status = manager.get_status("my-auth-service").await;
match status {
    HealthStatus::Healthy => println!("Service is healthy"),
    HealthStatus::Degraded => println!("Service is degraded"),
    HealthStatus::Unhealthy => println!("Service is unhealthy"),
}

// Check if service is available (not in cooldown)
let available = manager.is_available("my-auth-service").await;
```

#### Smart Selection with SQLite

```rust
use smart_routing::sqlite::SQLiteSelector;
use smart_routing::{SmartRoutingConfig, AuthInfo};

let config = SmartRoutingConfig::default();
let selector = SQLiteSelector::new(store, config);

let auths = vec![
    AuthInfo {
        id: "auth1".to_string(),
        priority: Some(100),
        quota_exceeded: false,
        unavailable: false,
        model_states: vec![],
    },
    AuthInfo {
        id: "auth2".to_string(),
        priority: Some(50),
        quota_exceeded: false,
        unavailable: false,
        model_states: vec![],
    },
];

// Select best auth based on weighted criteria
let selected = selector.pick(auths).await;
if let Some(auth_id) = selected {
    println!("Selected auth: {}", auth_id);
}

// Precompute weights for batch operations
let auth_ids = vec!["auth1".to_string(), "auth2".to_string()];
selector.precompute_weights(auth_ids).await?;

// Get selector statistics
let stats = selector.get_stats();
println!("Selections: {}", stats.select_count);
println!("DB queries: {}", stats.db_queries);
```

#### Weight Calculation

The SQLite selector uses SQL-based queries to calculate weights based on:

- **Success Rate**: Higher success rate = higher weight
- **Latency**: Lower latency = higher weight (normalized to max 500ms)
- **Health Status**: Healthy > Degraded > Unhealthy
- **Priority**: Configurable priority value (-100 to 100)
- **Quota Status**: Services with available quota preferred
- **Availability**: Unavailable services excluded

The weight formula:
```rust
weight = success_rate_weight * success_rate
       + latency_weight * latency_score
       + health_weight * health_factor
       + load_weight * load_score
       + priority_weight * priority_score
```

Penalties are applied for:
- Unhealthy services: `unhealthy_penalty`
- Degraded services: `degraded_penalty`
- Quota exceeded: `quota_exceeded_penalty`
- Unavailable: `unavailable_penalty`

#### Performance Considerations

- Use in-memory database (`:memory:`) for testing and development
- Use file-based database for production persistence
- SQLite is ideal for single-instance deployments
- For distributed deployments, consider implementing a PostgreSQL backend
- Connection pooling reduces connection overhead
- Indexes on auth_id ensure fast lookups

### Model Registry Configuration

```rust
use model_registry::{Registry, RegistryConfig};

let config = RegistryConfig {
    fetcher: Arc::new(MyFetcher::new()),
    ttl: chrono::Duration::hours(1),
    enable_background_refresh: true,
    refresh_interval: chrono::Duration::minutes(30),
};

let registry = Registry::with_config(config);
```

## Development

### Running Tests

```bash
# Run all tests in workspace
cargo test --workspace

# Run with clippy for linting
cargo clippy --workspace

# Format code
cargo fmt --all
```

### Building for Release

```bash
# Build optimized release binary
cargo build --release --package gateway

# Run release binary
./target/release/gateway
```

## Package Details

### smart-routing
- **Path**: `packages/smart-routing/`
- **Features**: Weight calculation, health management, smart selection
- **Key types**: `Router`, `SmartSelector`, `WeightCalculator`

### model-registry
- **Path**: `packages/model-registry/`
- **Features**: Model information caching, categorization, dynamic fetching
- **Key types**: `Registry`, `ModelInfo`, `ModelFetcher`

### tracing
- **Path**: `packages/tracing/`
- **Features**: Request tracing, metrics, middleware
- **Key types**: `TraceSpan`, `TraceCollector`, `TracingMiddleware`

## License

[Specify your license here]

## Contributing

Contributions are welcome! Please open an issue or submit a pull request.

## Status

This project is under active development. Some features are marked as "integration pending" and will be completed in future iterations.
