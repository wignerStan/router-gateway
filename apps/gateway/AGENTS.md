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
