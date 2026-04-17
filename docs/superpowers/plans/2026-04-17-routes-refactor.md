# Routes.rs Refactor: Common Helpers + Duplicate Removal

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reduce `tests/routes.rs` from 1590 lines to ~900 lines by removing middleware tests duplicated in `tests/middleware.rs` and refactoring remaining tests to use `tests/common/mod.rs` helpers.

**Architecture:** Two-phase approach: first remove duplicated tests (safe deletion), then refactor remaining tests to use shared helpers (mechanical replacement). Each phase produces a working commit with all tests passing.

**Tech Stack:** Rust, Axum 0.8, tower::ServiceExt, existing `tests/common/mod.rs` helpers

---

## Context for the Implementer

### What already exists

- **`tests/common/mod.rs`** (committed in `41a2c78`): Shared test helpers including:
  - `test_addr()` — returns `SocketAddr::from(([127, 0, 0, 1], 12345))`
  - `MAX_RESPONSE_BYTES`, `ERR_INVALID_REQUEST`, `ERR_CONFIG_ERROR`, `ERR_RATE_LIMIT`, `ERR_NO_ROUTE`
  - `read_json<T>(response)` — read and deserialize JSON body
  - `assert_status(response, expected)` — assert status, return response
  - `assert_json<T>(response, status)` — assert status + deserialize body
  - `RequestBuilder` — fluent builder: `get(uri)`, `post_json(uri, body)`, `with_auth(token)`, `with_connect_info(addr)`, `with_header(key, value)`, `build()`
  - `send(app, req)` — clone app + oneshot
  - `send_json<T>(app, req, status)` — send + assert_json

- **`tests/middleware.rs`** (committed in `41a2c78`): 11 isolated middleware tests covering auth (5), rate-limit (4), security-headers (2). Each middleware tested on a minimal router with an `ok_handler` that returns 200 "ok".

### What's duplicated

The following tests in `tests/routes.rs` test the same middleware behaviors as `tests/middleware.rs`:

| routes.rs test | lines | middleware.rs equivalent | reason to remove |
|---|---|---|---|
| `test_security_headers_present` | 305–345 | `all_security_headers_present` | identical test, minimal router |
| `test_rate_limiter_rejects_excess_requests` | 347–387 | `over_limit_returns_429` | identical test, minimal router |
| `test_valid_bearer_token` | 583–599 | `valid_bearer_token_passes` | identical test via full app |
| `test_invalid_token` | 601–627 | `invalid_token_returns_401` | identical test via full app |
| `test_missing_auth_header` | 629–653 | `missing_auth_returns_401` | identical test via full app |
| `test_wrong_auth_scheme` | 655–680 | N/A | **keep** — tests Basic auth scheme |
| `test_no_auth_tokens_configured` | 682–705 | `no_tokens_configured_returns_403` | identical test via full app |
| `test_second_token_valid` | 707–727 | `multiple_tokens_any_valid` | identical test via full app |
| `test_allows_requests_under_limit` | 756–765 | `under_limit_passes` | identical via full app |
| `test_blocks_requests_over_limit` | 768–790 | `over_limit_returns_429` | identical via full app |
| `test_independent_ip_buckets` | 793–838 | `different_ips_independent` | identical via full app |
| `test_rate_limit_response_body` | 841–871 | `over_limit_returns_429` | identical via full app |
| `test_rate_limit_applies_to_public_endpoints` | 873–907 | N/A | **move** to middleware_composition |

### What stays

These modules test full-app integration (not just middleware) and are NOT duplicated:
- `public_endpoints` (405–573) — root/health structure, credential counts, no-auth access
- `protected_endpoints` (914–1017) — auth required on /api/models, /api/route, /v1/chat/completions
- `chat_completions` (1023–1171) — gateway metadata, classification, provider selection
- `middleware_composition` (1177–1312) — middleware ordering, headers on error responses
- `provider_integration_tests` (1318–1590) — provider adapter selection, endpoint construction
- Top-level: `test_models_endpoint_with_credentials`, `test_health_endpoint_returns_status`, `test_root_endpoint`, `test_models_endpoint`, `test_request_classification`, `test_route_endpoint`, `test_rate_limiter_unit`

### Boilerplate stats (before refactor)

- 107 `expect("time went backwards")` calls
- 34 `Request::builder()` calls
- 51 `.oneshot(` calls
- 40 `ConnectInfo` insertions

Target: near-zero of each.

---

## File Structure

| File | Action | Responsibility |
|---|---|---|
| `tests/routes.rs` | Modify (1590 → ~900 lines) | Remove duplicates, refactor to common helpers |
| `tests/common/mod.rs` | No changes | Already has all helpers needed |
| `tests/middleware.rs` | No changes | Already covers removed tests |

---

## Task 1: Remove Duplicated Middleware Tests

**Files:**
- Modify: `tests/routes.rs`

This task removes 10 tests and 1 helper that are fully covered by `tests/middleware.rs`. One unique test (`test_wrong_auth_scheme`) and one composition test (`test_rate_limit_applies_to_public_endpoints`) are preserved by moving them to `mod middleware_composition`.

- [ ] **Step 1: Remove `test_security_headers_present` (lines 305–345)**

Delete the entire function from `#[tokio::test]` through the closing `}`.

- [ ] **Step 2: Remove `test_rate_limiter_rejects_excess_requests` (lines 347–387)**

Delete the entire function.

- [ ] **Step 3: Remove `mod auth_middleware` (lines 579–728)**

Delete the entire module including the comment block `// Auth middleware` and the `mod auth_middleware { ... }` block.

- [ ] **Step 4: Remove `mod rate_limiting` (lines 734–908) but save one test**

Delete the entire `mod rate_limiting { ... }` block including the `exhaust_rate_limit` helper, BUT copy `test_rate_limit_applies_to_public_endpoints` (lines 873–907) to clipboard — it will be moved to `mod middleware_composition` in the next step.

- [ ] **Step 5: Move saved test into `mod middleware_composition`**

Add `test_rate_limit_applies_to_public_endpoints` at the end of `mod middleware_composition` (before the closing `}`). Adapt it: the test uses `create_test_state_overrides`, `build_full_app`, `common::test_addr()` — all available in the `middleware_composition` module via `use super::*;`.

The moved test body should look like:

```rust
#[tokio::test]
async fn test_rate_limit_applies_to_public_endpoints() {
    let state = create_test_state_overrides(TestOverrides {
        rate_limit: Some(1),
        ..TestOverrides::default()
    });
    let app = build_full_app(state);
    let test_addr = common::test_addr();

    // Exhaust the limit on /health
    let response = common::send(
        &app,
        common::RequestBuilder::get("/health")
            .with_connect_info(test_addr)
            .build(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    // Both /health and / should be rate-limited
    let response = common::send(
        &app,
        common::RequestBuilder::get("/health")
            .with_connect_info(test_addr)
            .build(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);

    let response = common::send(
        &app,
        common::RequestBuilder::get("/")
            .with_connect_info(test_addr)
            .build(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
}
```

Note: this uses the common helpers pattern that will be wired up in Task 2. If the import isn't ready yet, use the existing verbose pattern and refactor it in Task 3.

- [ ] **Step 6: Verify tests pass**

Run: `cargo nextest run --test routes`
Expected: All remaining tests pass. Should be fewer tests than before but all green.

- [ ] **Step 7: Commit**

```bash
git add tests/routes.rs
git commit -m "refactor: remove middleware tests duplicated in tests/middleware.rs"
```

---

## Task 2: Wire Up Common Module, Remove Local Helpers

**Files:**
- Modify: `tests/routes.rs`

- [ ] **Step 1: Add `mod common;` import**

Add `mod common;` right after the file-level `#![allow(...)]` attribute, before the `use` block:

```rust
#![allow(
    missing_docs,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::needless_pass_by_value,
    clippy::panic
)]
mod common;
use axum::{
    // ... existing imports
```

- [ ] **Step 2: Remove local constants and helper**

Delete these items from routes.rs (they are now in `common`):
- `const MAX_RESPONSE_BYTES` (line ~40)
- `const ERR_INVALID_REQUEST` (line ~42)
- `const ERR_CONFIG_ERROR` (line ~43)
- `const ERR_RATE_LIMIT` (line ~44)
- `const ERR_NO_ROUTE` (line ~45)
- `async fn read_json_body<T>(...)` (lines ~47–57)

- [ ] **Step 3: Verify tests compile**

Run: `cargo check --test routes`
Expected: Compilation fails because references to removed items need to be updated. This is expected — fix in Step 4.

- [ ] **Step 4: Replace all references to removed items**

Replace throughout the file:
- `read_json_body(response).await` → `common::read_json::<Type>(response).await`
- `MAX_RESPONSE_BYTES` → `common::MAX_RESPONSE_BYTES`
- `ERR_INVALID_REQUEST` → `common::ERR_INVALID_REQUEST`
- `ERR_CONFIG_ERROR` → `common::ERR_CONFIG_ERROR`
- `ERR_RATE_LIMIT` → `common::ERR_RATE_LIMIT`
- `ERR_NO_ROUTE` → `common::ERR_NO_ROUTE`

Note: after removal, `MAX_RESPONSE_BYTES` should have zero references since `read_json_body` was the only user.

- [ ] **Step 5: Verify tests pass**

Run: `cargo nextest run --test routes`
Expected: All tests pass. Behavior unchanged.

- [ ] **Step 6: Commit**

```bash
git add tests/routes.rs
git commit -m "refactor: use tests/common helpers in routes.rs"
```

---

## Task 3: Refactor Request Building Patterns — Top-Level Tests

**Files:**
- Modify: `tests/routes.rs`

Refactor the 7 top-level tests (outside any `mod` block) to use `common::RequestBuilder`, `common::send`, and `common::send_json`.

- [ ] **Step 1: Refactor `test_models_endpoint_with_credentials`**

Replace:
```rust
let response = app
    .oneshot(
        Request::builder()
            .uri("/api/models")
            .body(Body::empty())
            .expect("time went backwards"),
    )
    .await
    .expect("time went backwards");
assert_eq!(response.status(), StatusCode::OK);
let list: serde_json::Value = read_json_body(response).await;
```

With:
```rust
let list: serde_json::Value = common::send_json(
    &app,
    common::RequestBuilder::get("/api/models").build(),
    StatusCode::OK,
)
.await;
```

- [ ] **Step 2: Refactor `test_health_endpoint_returns_status`**

Replace the oneshot+assert+read pattern with:
```rust
let health: HealthStatus = common::send_json(
    &app,
    common::RequestBuilder::get("/health").build(),
    StatusCode::OK,
)
.await;
assert_eq!(health.status, "healthy");
```

- [ ] **Step 3: Refactor `test_root_endpoint`**

Replace with:
```rust
let response = common::send(
    &app,
    common::RequestBuilder::get("/").build(),
)
.await;
assert_eq!(response.status(), StatusCode::OK);
```

Note: `Router::new().route("/", get(root))` doesn't need `with_state`, so `common::send` still works.

- [ ] **Step 4: Refactor `test_models_endpoint`**

Replace with:
```rust
let response = common::send(
    &app,
    common::RequestBuilder::get("/api/models").build(),
)
.await;
assert_eq!(response.status(), StatusCode::OK);
```

- [ ] **Step 5: Leave `test_request_classification` unchanged**

This test doesn't use HTTP requests — it tests the classifier directly. No changes needed.

- [ ] **Step 6: Refactor `test_route_endpoint`**

Replace with:
```rust
let response = common::send(
    &app,
    common::RequestBuilder::get("/api/route").build(),
)
.await;
assert_eq!(response.status(), StatusCode::OK);
```

- [ ] **Step 7: Leave `test_rate_limiter_unit` unchanged**

This test doesn't use HTTP requests — it tests `RateLimiter::check()` directly. No changes needed.

- [ ] **Step 8: Verify tests pass**

Run: `cargo nextest run --test routes`
Expected: All tests pass.

- [ ] **Step 9: Commit**

```bash
git add tests/routes.rs
git commit -m "refactor: use common request builders in top-level route tests"
```

---

## Task 4: Refactor `mod public_endpoints`

**Files:**
- Modify: `tests/routes.rs` (lines ~405–573 in original, shifted after Task 1 deletions)

Each test in this module follows the pattern:
```rust
let mut request = Request::builder()
    .uri("/health")
    .body(Body::empty())
    .expect("time went backwards");
request.extensions_mut()
    .insert(axum::extract::ConnectInfo(test_addr));
let response = app.oneshot(request).await.expect("time went backwards");
assert_eq!(response.status(), StatusCode::OK);
let health: HealthStatus = read_json_body(response).await;
```

Becomes:
```rust
let health: HealthStatus = common::send_json(
    &app,
    common::RequestBuilder::get("/health")
        .with_connect_info(common::test_addr())
        .build(),
    StatusCode::OK,
)
.await;
```

- [ ] **Step 1: Refactor `test_root_response_structure`**

Replace request building + oneshot with `common::send`. The response body assertion stays.

- [ ] **Step 2: Refactor `test_health_response_structure`**

Replace with `common::send_json`.

- [ ] **Step 3: Refactor `test_health_with_credentials`**

Replace with `common::send_json`.

- [ ] **Step 4: Refactor `test_health_credential_counts`**

Replace with `common::send_json`.

- [ ] **Step 5: Refactor `test_public_endpoints_no_auth`**

This test has two requests. Replace both with `common::send`. The first uses `app.clone()` implicitly via `common::send` (which clones internally).

- [ ] **Step 6: Verify tests pass**

Run: `cargo nextest run --test routes`
Expected: All tests pass.

- [ ] **Step 7: Commit**

```bash
git add tests/routes.rs
git commit -m "refactor: use common helpers in public_endpoints tests"
```

---

## Task 5: Refactor `mod protected_endpoints`

**Files:**
- Modify: `tests/routes.rs`

- [ ] **Step 1: Refactor `test_models_requires_auth`**

Replace:
```rust
let mut request = Request::builder()
    .uri("/api/models")
    .body(Body::empty())
    .expect("time went backwards");
request.extensions_mut()
    .insert(axum::extract::ConnectInfo(test_addr));
let response = app.oneshot(request).await.expect("time went backwards");
assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
```

With:
```rust
let response = common::send(
    &app,
    common::RequestBuilder::get("/api/models")
        .with_connect_info(common::test_addr())
        .build(),
)
.await;
assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
```

- [ ] **Step 2: Refactor `test_route_requires_auth`**

Same pattern as Step 1 but for `/api/route`.

- [ ] **Step 3: Refactor `test_chat_completions_requires_auth`**

This test has a POST body. Replace:
```rust
let body = Body::from(
    serde_json::to_string(&json!({...})).expect("time went backwards"),
);
let mut request = Request::builder()
    .method("POST")
    .uri("/v1/chat/completions")
    .header("content-type", "application/json")
    .body(body)
    .expect("time went backwards");
request.extensions_mut()
    .insert(axum::extract::ConnectInfo(test_addr));
let response = app.oneshot(request).await.expect("time went backwards");
```

With:
```rust
let response = common::send(
    &app,
    common::RequestBuilder::post_json(
        "/v1/chat/completions",
        &json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "hello"}]
        }),
    )
    .with_connect_info(common::test_addr())
    .build(),
)
.await;
```

- [ ] **Step 4: Refactor `test_models_with_auth_returns_list`**

Replace with:
```rust
let value: serde_json::Value = common::send_json(
    &app,
    common::RequestBuilder::get("/api/models")
        .with_auth("test-token")
        .with_connect_info(common::test_addr())
        .build(),
    StatusCode::OK,
)
.await;
```

- [ ] **Step 5: Verify tests pass**

Run: `cargo nextest run --test routes`
Expected: All tests pass.

- [ ] **Step 6: Commit**

```bash
git add tests/routes.rs
git commit -m "refactor: use common helpers in protected_endpoints tests"
```

---

## Task 6: Refactor `mod chat_completions`

**Files:**
- Modify: `tests/routes.rs`

This module has its own helpers: `chat_request_body()`, `make_chat_request()`.

- [ ] **Step 1: Refactor `make_chat_request` helper**

Replace:
```rust
fn make_chat_request(addr: std::net::SocketAddr) -> Request<Body> {
    let mut request = Request::builder()
        .method("POST")
        .uri("/v1/chat/completions")
        .header("content-type", "application/json")
        .header("authorization", "Bearer test-token")
        .body(chat_request_body())
        .expect("time went backwards");
    request.extensions_mut()
        .insert(axum::extract::ConnectInfo(addr));
    request
}
```

With:
```rust
fn make_chat_request(addr: std::net::SocketAddr) -> Request<Body> {
    common::RequestBuilder::post_json(
        "/v1/chat/completions",
        &json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hello"}]
        }),
    )
    .with_auth("test-token")
    .with_connect_info(addr)
    .build()
}
```

Remove `chat_request_body()` — it's now inline.

- [ ] **Step 2: Refactor individual test assertions**

For each test in the module, replace:
```rust
let response = app.oneshot(make_chat_request(test_addr)).await.expect("time went backwards");
assert_eq!(response.status(), StatusCode::OK);
let value = read_json_body::<serde_json::Value>(response).await;
```

With:
```rust
let value: serde_json::Value = common::send_json(
    &app,
    make_chat_request(common::test_addr()),
    StatusCode::OK,
)
.await;
```

And for non-OK status:
```rust
let value: serde_json::Value = common::send_json(
    &app,
    make_chat_request(common::test_addr()),
    StatusCode::SERVICE_UNAVAILABLE,
)
.await;
```

Apply this pattern to:
- `test_response_structure`
- `test_no_credentials_returns_503`
- `test_gateway_metadata`
- `test_classification_in_response`

- [ ] **Step 3: Remove `test_addr` local variable from tests**

Each test currently creates `let test_addr = std::net::SocketAddr::from(([127, 0, 0, 1], 12345));`. After refactoring `make_chat_request`, replace `test_addr` with `common::test_addr()`.

- [ ] **Step 4: Verify tests pass**

Run: `cargo nextest run --test routes`
Expected: All tests pass.

- [ ] **Step 5: Commit**

```bash
git add tests/routes.rs
git commit -m "refactor: use common helpers in chat_completions tests"
```

---

## Task 7: Refactor `mod middleware_composition`

**Files:**
- Modify: `tests/routes.rs`

- [ ] **Step 1: Refactor `test_security_headers_on_all_responses`**

Replace both request+oneshot blocks with `common::send`. The second request has auth:
```rust
let response = common::send(
    &app,
    common::RequestBuilder::get("/health")
        .with_connect_info(common::test_addr())
        .build(),
)
.await;
check_security_headers(response.headers());

let response = common::send(
    &app,
    common::RequestBuilder::get("/api/models")
        .with_auth("test-token")
        .with_connect_info(common::test_addr())
        .build(),
)
.await;
check_security_headers(response.headers());
```

- [ ] **Step 2: Refactor `test_security_headers_on_error_responses`**

Replace with:
```rust
let response = common::send(
    &app,
    common::RequestBuilder::get("/api/models")
        .with_connect_info(common::test_addr())
        .build(),
)
.await;
assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
check_security_headers(response.headers());
```

- [ ] **Step 3: Refactor `test_auth_before_handler`**

Replace with:
```rust
let response = common::send(
    &app,
    common::RequestBuilder::get("/api/models")
        .with_auth("invalid-token")
        .with_connect_info(common::test_addr())
        .build(),
)
.await;
assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
```

- [ ] **Step 4: Refactor `test_rate_limit_before_auth`**

Replace the two request+oneshot blocks:
```rust
let response = common::send(
    &app,
    common::RequestBuilder::get("/health")
        .with_connect_info(common::test_addr())
        .build(),
)
.await;
assert_eq!(response.status(), StatusCode::OK);

let response = common::send(
    &app,
    common::RequestBuilder::get("/api/models")
        .with_connect_info(common::test_addr())
        .build(),
)
.await;
assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);

let value: serde_json::Value = common::read_json(response).await;
assert_eq!(value["error"]["type"], common::ERR_RATE_LIMIT);
```

- [ ] **Step 5: Verify tests pass**

Run: `cargo nextest run --test routes`
Expected: All tests pass.

- [ ] **Step 6: Commit**

```bash
git add tests/routes.rs
git commit -m "refactor: use common helpers in middleware_composition tests"
```

---

## Task 8: Refactor `mod provider_integration_tests`

**Files:**
- Modify: `tests/routes.rs`

This module has its own helpers: `make_credential`, `make_chat_request`, `create_routing_state`.

- [ ] **Step 1: Refactor `make_chat_request` in this module**

Replace:
```rust
fn make_chat_request(auth_token: &str, body: serde_json::Value) -> Request<Body> {
    let body_bytes = serde_json::to_vec(&body).expect("time went backwards");
    let mut req = Request::builder()
        .method("POST")
        .uri("/v1/chat/completions")
        .header("Authorization", format!("Bearer {auth_token}"))
        .header("Content-Type", "application/json")
        .body(Body::from(body_bytes))
        .expect("time went backwards");
    let test_addr = std::net::SocketAddr::from(([127, 0, 0, 1], 12345));
    req.extensions_mut()
        .insert(axum::extract::ConnectInfo(test_addr));
    req
}
```

With:
```rust
fn make_chat_request(auth_token: &str, body: &serde_json::Value) -> Request<Body> {
    common::RequestBuilder::post_json("/v1/chat/completions", body)
        .with_auth(auth_token)
        .with_connect_info(common::test_addr())
        .build()
}
```

- [ ] **Step 2: Refactor each test's oneshot+assert pattern**

For each test, replace:
```rust
let response = app.oneshot(req).await.expect("time went backwards");
assert_eq!(response.status(), StatusCode::OK);
let json_body = read_json_body::<serde_json::Value>(response).await;
```

With:
```rust
let json_body: serde_json::Value = common::send_json(&app, req, StatusCode::OK).await;
```

Apply to all 9 tests in the module:
- `test_openai_adapter_selected`
- `test_google_adapter_selected`
- `test_deepseek_uses_openai_adapter`
- `test_unknown_provider_is_routed_successfully`
- `test_openai_endpoint_default`
- `test_openai_endpoint_custom_base_url`
- `test_google_endpoint_includes_model`
- `test_chat_completions_with_temperature_and_max_tokens`
- `test_chat_completions_vision_request`

- [ ] **Step 3: Update call sites for `make_chat_request` signature change**

Since the signature changed from `(auth_token: &str, body: serde_json::Value)` to `(auth_token: &str, body: &serde_json::Value)`, update all call sites to pass `&json!(...)` instead of `json!(...)`:

```rust
// Before:
let req = make_chat_request("test-token", json!({...}));

// After:
let req = make_chat_request("test-token", &json!({...}));
```

- [ ] **Step 4: Verify tests pass**

Run: `cargo nextest run --test routes`
Expected: All tests pass.

- [ ] **Step 5: Commit**

```bash
git add tests/routes.rs
git commit -m "refactor: use common helpers in provider_integration tests"
```

---

## Task 9: Final Cleanup

**Files:**
- Modify: `tests/routes.rs`

- [ ] **Step 1: Remove unused imports**

After all refactoring, some imports may be unused. Check and remove:
- `axum::body::Body` — likely still needed for `Request<Body>` return types in helpers
- `axum::http::Request` — may be unused if all request building goes through `common::RequestBuilder`
- `tower::ServiceExt` — may be unused if all oneshots go through `common::send`

Run: `cargo check --test routes 2>&1 | grep "unused import"`
Remove any flagged imports.

- [ ] **Step 2: Run clippy**

Run: `cargo clippy --test routes -- -D warnings`
Fix any warnings.

- [ ] **Step 3: Run format**

Run: `cargo fmt --all`

- [ ] **Step 4: Run full test suite**

Run: `cargo nextest run`
Expected: All tests across all files pass (routes, middleware, common, provider_integration, etc.)

- [ ] **Step 5: Verify line count reduction**

Run: `wc -l tests/routes.rs`
Expected: ~900–1000 lines (down from 1590).

Run: `grep -c 'expect("time went backwards")' tests/routes.rs`
Expected: 0 or near-0.

Run: `grep -c 'Request::builder()' tests/routes.rs`
Expected: 0 (all replaced by `common::RequestBuilder`).

- [ ] **Step 6: Commit**

```bash
git add tests/routes.rs
git commit -m "refactor: clean up unused imports and format routes.rs"
```

---

## Verification Checklist

After all tasks are complete:

```bash
# All tests pass
cargo nextest run

# Zero clippy warnings
cargo clippy --all-targets -- -D warnings

# Clean format
cargo fmt --all -- --check

# Line count ~900-1000
wc -l tests/routes.rs

# Near-zero boilerplate
grep -c 'expect("time went backwards")' tests/routes.rs  # expect: 0
grep -c 'Request::builder()' tests/routes.rs             # expect: 0
grep -c '\.oneshot(' tests/routes.rs                      # expect: 0
```
