# Gateway

A local LLM gateway written in Rust for intelligent request routing. Routes LLM requests to optimal credentials based on health, latency, success rate, and configurable policies. Designed for local development and self-hosted deployments.

## Architecture

```
                           Request
                             │
                             ▼
┌─────────────────────────────────────────────────────────────┐
│                      Middleware Stack                         │
│                                                             │
│  Security Headers  │  Rate Limiter  │  LLM Tracing  │ Auth │
└─────────────────────────────────────────────────────────────┘
                             │
              ┌──────────────┼──────────────┐
              ▼              ▼              ▼
        Public Routes  Protected Routes  Chat Completions
        GET  /          GET  /api/models  POST /v1/chat/completions
        GET  /health    GET  /api/route
                             │
                             ▼
┌─────────────────────────────────────────────────────────────┐
│                      Core Services                           │
│                                                             │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐  │
│  │ Smart        │  │ Model        │  │ LLM              │  │
│  │ Routing      │  │ Registry     │  │ Tracing          │  │
│  │              │  │              │  │                  │  │
│  │ Strategies:  │  │ 5-dimension  │  │ TraceSpan        │  │
│  │ weighted     │  │ categorize   │  │ Metrics          │  │
│  │ time_aware   │  │ 20+ providers│  │ Middleware       │  │
│  │ quota_aware  │  │ Policy rules │  │                  │  │
│  │ adaptive     │  │              │  │                  │  │
│  └──────────────┘  └──────────────┘  └──────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                             │
                             ▼
                    ┌─────────────────┐
                    │  External LLMs  │
                    │  (20+ providers)│
                    └─────────────────┘
```

## Features

### Smart Routing

Policy-based credential selection with five routing strategies and configurable weight factors.

**Strategies** (configured via `routing.strategy` in gateway.yaml):

| Strategy       | Purpose                                                       |
| -------------- | ------------------------------------------------------------- |
| `weighted`     | Weighted random selection based on composite scores (default) |
| `time_aware`   | Time-based credential preference with peak/off-peak slots     |
| `quota_aware`  | Quota-balanced selection with reserve ratio                   |
| `adaptive`     | Dynamically adjusts based on real-time metrics                |
| `policy_aware` | Route based on model-registry policy rules                    |

**Weight factors** (configurable via `SmartRoutingConfig.weight`):

| Factor       | Default | Purpose                                    |
| ------------ | ------- | ------------------------------------------ |
| success_rate | 0.35    | Favor credentials with higher success rate |
| latency      | 0.25    | Favor lower-latency credentials            |
| health       | 0.20    | Prefer healthy over degraded/unhealthy     |
| load         | 0.15    | Balance load across credentials            |
| priority     | 0.05    | Manual priority override                   |

**Health state machine**: `Healthy` -> `Degraded` (429/503) -> `Unhealthy` (401-403/500/502/504), with configurable cooldown periods and recovery thresholds.

### Model Registry

Multi-dimension categorization for routing decisions with a built-in routing policy system.

| Dimension      | Categories                                 | Purpose             |
| -------------- | ------------------------------------------ | ------------------- |
| **Capability** | Vision, Tools, Streaming, Thinking         | Feature matching    |
| **Tier**       | Flagship, Standard, Fast                   | Quality routing     |
| **Cost**       | UltraPremium, Premium, Standard, Economy   | Cost optimization   |
| **Context**    | Small (<32K) through Ultra (500K+)         | Context fitting     |
| **Provider**   | 20+ providers                              | Vendor routing      |
| **Modality**   | Text, Image, Audio, Video, Embedding, Code | Multi-modal routing |

**Routing policies** support filters (capabilities, tiers, costs, providers, modalities), actions (prefer, avoid, block, weight), and conditional application (time-based, tenant-based, token-count). Policies are validated against a JSON schema (`config/policies.schema.json`).

### LLM Tracing

Request/response observability via `TraceSpan`, `MemoryTraceCollector`, and `TracingMiddleware`. Tracks request ID, provider, model, tokens, latency, and errors with in-memory collection.

### Security

- **SSRF protection**: Blocks private/reserved IPs (loopback, link-local, cloud metadata), including IPv4-mapped and IPv4-compatible IPv6 addresses
- **Constant-time token comparison**: Uses `subtle::ConstantTimeEq` to prevent timing side-channels on auth tokens
- **Rate limiting**: Per-IP rate limiting with configurable limits and periodic bucket pruning
- **Security headers**: `X-Content-Type-Options`, `X-Frame-Options`, `Referrer-Policy`, `Content-Security-Policy` on all responses
- **Fail-closed auth**: Rejects requests when no auth tokens are configured (unless `GATEWAY_ENV=development`)

## Project Structure

```
gateway/
  crates/
    gateway/                 # HTTP API server (Axum) — lib + bin
    model-registry/          # Model metadata, 5-dimension categorization
    smart-routing/           # Weighted credential selection, health tracking
    llm-tracing/             # Request/response observability
  cli/                       # CLI management utility (package: my-cli)
  deploy/                    # Docker deployment files
  config/
    policies.json            # Routing policy configuration
    policies.schema.json     # JSON schema for policy validation
```

## Getting Started

### Prerequisites

- Rust 1.85 or later
- Cargo (included with Rust)

### Build

```bash
cargo build
```

### Run

```bash
# Run the gateway (starts on 0.0.0.0:3000 by default)
cargo run -p gateway

# Run with debug logging
RUST_LOG=debug cargo run -p gateway

# Run with a specific config file
GATEWAY_CONFIG=./gateway.yaml cargo run -p gateway
```

### Configuration

The gateway loads configuration from (in order of precedence):

1. `GATEWAY_CONFIG` environment variable
2. `./gateway.yaml`
3. `./config/gateway.yaml`
4. `./gateway.yml`

If no config file is found, defaults are used. Environment variable interpolation is supported in secret fields using `${VAR_NAME}` and `${VAR_NAME:-default}` syntax.

```yaml
server:
  port: 3000
  host: "0.0.0.0"
  timeout_secs: 120
  auth_tokens:
    - "${GATEWAY_AUTH_TOKEN}"
  trust_proxy_headers: false

credentials:
  - id: openai-primary
    provider: openai
    api_key: "${OPENAI_API_KEY}"
    priority: 10
    allowed_models:
      - gpt-4o
      - gpt-4o-mini
  - id: anthropic-primary
    provider: anthropic
    api_key: "${ANTHROPIC_API_KEY}"
    priority: 20
    daily_quota: 10000

routing:
  strategy: weighted
  session_affinity: true
  min_healthy_credentials: 1
  fallback_depth: 2

providers:
  openai:
    enabled: true
    base_url: "https://api.openai.com"
```

### Environment Variables

| Variable         | Description                                                           | Default                                     |
| ---------------- | --------------------------------------------------------------------- | ------------------------------------------- |
| `GATEWAY_CONFIG` | Path to YAML configuration file                                       | auto-detected                               |
| `RUST_LOG`       | Log level filter                                                      | `gateway=debug,tower_http=debug,axum=debug` |
| `GATEWAY_ENV`    | Environment mode (`development` skips auth when no tokens configured) | -                                           |

## API Endpoints

### Public (no authentication required)

#### `GET /`

Returns gateway info including name, version, and available features.

```json
{
  "name": "Gateway API",
  "version": "0.1.0",
  "description": "Smart routing gateway for LLM requests",
  "features": ["Smart Routing", "Model Registry", "LLM Tracing", "Health Management"]
}
```

#### `GET /health`

Health check with credential status counts and uptime.

```json
{
  "status": "healthy",
  "uptime_secs": 3600,
  "credential_count": 3,
  "healthy_count": 3,
  "degraded_count": 0,
  "unhealthy_count": 0
}
```

### Protected (requires `Authorization: Bearer <token>`)

#### `GET /api/models`

Lists available models from configured credentials.

#### `GET /api/route`

Plans a route for a sample request, returning the primary route and fallback chain with classification details.

#### `POST /v1/chat/completions`

OpenAI-compatible chat completions endpoint. Proxies requests to the selected provider after classifying the request and planning the optimal route.

## Development

### Quick Reference

| Task     | Command                     |
| -------- | --------------------------- |
| Build    | `cargo build`               |
| Test     | `cargo test --workspace`    |
| Run      | `cargo run -p gateway`      |
| Dev mode | `just dev`                  |
| Lint     | `cargo clippy --workspace`  |
| Format   | `cargo fmt`                 |
| Fast QA  | `just qa`                   |
| Full QA  | `just qa-full`              |
| Docs     | `cargo doc --no-deps --all` |

### Quality Gates

The project uses a tiered verification system via `just`:

- **Tier 1** (`just qa`): Format check, fast clippy, type check -- for pre-commit
- **Tier 2** (`just qa-full`): Tier 1 + tests + security audit -- for pre-push/CI

### Workspace Lints

The workspace enforces strict lints at the workspace level: `clippy::all`, `clippy::pedantic`, `clippy::perf` (deny), and `clippy::nursery` (warn). `unwrap()`, `expect()`, and `panic!` are denied in production code. `unsafe_code` is forbidden.

## Package Reference

### `smart-routing` (`crates/smart-routing/`)

Weighted credential selection with health tracking, time-aware routing, and policy-based dispatch.

| Type                 | Location                   |
| -------------------- | -------------------------- |
| `SmartRoutingConfig` | `src/config/mod.rs`        |
| `WeightConfig`       | `src/config/mod.rs`        |
| `HealthManager`      | `src/health/mod.rs`        |
| `WeightCalculator`   | `src/weight/calculator.rs` |
| `Router`             | `src/router/mod.rs`        |
| `MetricsCollector`   | `src/metrics.rs`           |

### `model-registry` (`crates/model-registry/`)

Model metadata, 5-dimension categorization, and routing policy system with JSON schema validation.

| Type                  | Location                 |
| --------------------- | ------------------------ |
| `ModelInfo`           | `src/info.rs`            |
| `Registry`            | `src/registry/mod.rs`    |
| `ModelCategorization` | `src/categories.rs`      |
| `RoutingPolicy`       | `src/policy/mod.rs`      |
| `PolicyRegistry`      | `src/policy/registry.rs` |

### `llm-tracing` (`crates/llm-tracing/`)

Request/response observability with in-memory trace collection and Axum middleware integration.

| Type                   | Location            |
| ---------------------- | ------------------- |
| `TraceSpan`            | `src/trace.rs`      |
| `MemoryTraceCollector` | `src/collector.rs`  |
| `TracingMiddleware`    | `src/middleware.rs` |

### `gateway` (`crates/gateway/`)

HTTP API server with middleware stack, provider adapters, and route handlers.

| Type              | Location         |
| ----------------- | ---------------- |
| `GatewayConfig`   | `src/config.rs`  |
| `AppState`        | `src/state.rs`   |
| Route handlers    | `src/routes.rs`  |
| Provider adapters | `src/providers/` |

## License

MIT OR Apache-2.0
