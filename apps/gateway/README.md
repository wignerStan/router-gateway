# gateway

HTTP API server for the smart routing gateway.

## Purpose

Main HTTP server exposing the gateway API for LLM request routing.

## Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/` | GET | API info |
| `/health` | GET | Health check |
| `/api/models` | GET | List available models |
| `/api/route` | GET | Route a request |

## Configuration

Environment variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `RUST_LOG` | `gateway=debug` | Log level |
| `GATEWAY_PORT` | `3000` | HTTP server port |

## Usage

```bash
# Run with defaults
cargo run -p gateway

# Run with debug logging
RUST_LOG=debug cargo run -p gateway

# Run with custom port
GATEWAY_PORT=8080 cargo run -p gateway
```

## Architecture

```
main.rs
  ├── Tracing setup
  ├── Model registry initialization
  ├── Smart router initialization
  ├── Route definitions
  │   ├── root()
  │   ├── health_check()
  │   ├── list_models()
  │   └── route_request()
  └── Integration tests
```

## Dependencies

- `axum` - HTTP framework
- `tower-http` - Middleware (tracing)
- `tokio` - Async runtime
- `serde_json` - JSON handling
- `model-registry` - Model info
- `smart-routing` - Routing logic
- `llm-tracing` - Observability

## Tests

```bash
cargo test -p gateway
```
