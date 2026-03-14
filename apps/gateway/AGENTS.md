# gateway

This project is part of the workspace. Please refer to the root [AGENTS.md](../../AGENTS.md) for global project guidelines and best practices.

## Crate-Specific Instructions

- **Location:** `apps/gateway`
- **Package:** `gateway`
- **Build:** Run `cargo build -p gateway`
- **Test:** Run `cargo test -p gateway`
- **Run:** `cargo run -p gateway` (starts on 0.0.0.0:3000)

## Key Facts

- HTTP API server built with Axum, Tokio
- Five endpoints: `GET /`, `GET /health`, `GET /api/models`, `GET /api/route`, `POST /v1/chat/completions`
- Protected routes require `Authorization: Bearer <token>` (fail-closed by default)
- Auth bypassed in development mode when no tokens configured (`GATEWAY_ENV=development`)
- Rate limiting: 60 req/min per IP (configurable)
- Three provider adapters: OpenAI, Google, Anthropic (in `src/providers/`)
- Configuration loaded from `gateway.yaml`, `config/gateway.yaml`, or `GATEWAY_CONFIG` env var
- Uses `constant_time_token_eq()` for all auth token comparisons (timing safety)

## Known Pitfalls

- Test temp files: Use `tempfile::NamedTempFile` for RAII cleanup — never manually write to `std::env::temp_dir()` with manual deletion. Extract repeated file-write-parse patterns into shared test helpers.
- Test API keys: Never use `sk-` prefixed strings in tests — use clearly non-production values like `test-key-123` to avoid triggering security scanners.
- Prefer `assert_eq!` for boolean JSON assertions over `assert!(expr.as_bool().unwrap_or(false), ...)` — it's more concise and provides clearer failure messages.
- Middleware ordering tests: Remove auth headers when testing rate-limit-before-auth — a valid token masks the regression (429 would fire regardless of middleware order).
- Integration test helpers: Extract repeated request setup (app build + request construction) into shared helper functions to reduce boilerplate and improve test maintenance.
- The `chat_completions` handler currently omits `_gateway.classification.capabilities.thinking` (returns `null`), which is inconsistent with the `RequiredCapabilities` struct — document this in tests with explicit null assertions
