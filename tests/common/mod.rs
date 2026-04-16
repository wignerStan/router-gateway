//! Shared test support module for integration tests.
//!
//! Provides request builders, response helpers, and common constants
//! to reduce repetition across the `tests/` directory.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Extensions, Request, StatusCode};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::net::SocketAddr;
use tower::ServiceExt;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum bytes to read from a response body.
pub const MAX_RESPONSE_BYTES: usize = 4096;

/// Default loopback address for tests that inject `ConnectInfo`.
pub fn test_addr() -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], 12345))
}

/// Error type string for invalid or expired auth tokens.
#[allow(dead_code)] // Public API for test consumers
pub const ERR_INVALID_REQUEST: &str = "invalid_request_error";

/// Error type string for configuration errors (e.g. no auth tokens).
#[allow(dead_code)] // Public API for test consumers
pub const ERR_CONFIG_ERROR: &str = "config_error";

/// Error type string for rate-limit responses.
pub const ERR_RATE_LIMIT: &str = "rate_limit_error";

/// Error type string when no suitable route is available.
pub const ERR_NO_ROUTE: &str = "no_route_available";

// ---------------------------------------------------------------------------
// Response helpers
// ---------------------------------------------------------------------------

/// Read the full response body and deserialize it as JSON.
///
/// Panics with the raw body text if deserialization fails.
pub async fn read_json<T: DeserializeOwned>(response: axum::response::Response) -> T {
    let body_bytes = axum::body::to_bytes(response.into_body(), MAX_RESPONSE_BYTES)
        .await
        .expect("response body should be readable");

    serde_json::from_slice(&body_bytes).unwrap_or_else(|e| {
        panic!(
            "Failed to deserialize JSON: {e}. Body: {}",
            String::from_utf8_lossy(&body_bytes)
        )
    })
}

/// Assert that the response status matches `expected`, then return the
/// response for further inspection (e.g. reading headers or body).
pub fn assert_status(
    response: axum::response::Response,
    expected: StatusCode,
) -> axum::response::Response {
    let actual = response.status();
    assert_eq!(actual, expected, "expected {expected}, got {actual}");
    response
}

/// Assert the response status and deserialize the body as JSON in one step.
pub async fn assert_json<T: DeserializeOwned>(
    response: axum::response::Response,
    expected: StatusCode,
) -> T {
    let response = assert_status(response, expected);
    read_json(response).await
}

// ---------------------------------------------------------------------------
// Request builder
// ---------------------------------------------------------------------------

/// Fluent request builder for constructing test requests with less boilerplate.
pub struct RequestBuilder {
    inner: axum::http::request::Builder,
    body: Body,
    extensions: Extensions,
}

impl RequestBuilder {
    /// Create a GET request to `uri`.
    pub fn get(uri: &str) -> Self {
        Self {
            inner: Request::builder().uri(uri),
            body: Body::empty(),
            extensions: Extensions::new(),
        }
    }

    /// Create a POST request to `uri` with a JSON-serialized body.
    pub fn post_json(uri: &str, body: &impl Serialize) -> Self {
        let bytes = serde_json::to_vec(body).expect("request body should serialize");
        Self {
            inner: Request::builder()
                .method("POST")
                .uri(uri)
                .header("content-type", "application/json"),
            body: Body::from(bytes),
            extensions: Extensions::new(),
        }
    }

    /// Add an `Authorization: Bearer {token}` header.
    pub fn with_auth(self, token: &str) -> Self {
        Self {
            inner: self
                .inner
                .header("authorization", format!("Bearer {token}")),
            body: self.body,
            extensions: self.extensions,
        }
    }

    /// Insert a `ConnectInfo` extension (used by rate-limit and auth middleware).
    pub fn with_connect_info(mut self, addr: SocketAddr) -> Self {
        self.extensions.insert(ConnectInfo(addr));
        self
    }

    /// Add an arbitrary header.
    #[allow(dead_code)] // Public API for test consumers
    pub fn with_header(self, key: &str, value: &str) -> Self {
        Self {
            inner: self.inner.header(key, value),
            body: self.body,
            extensions: self.extensions,
        }
    }

    /// Finalize the builder into a `Request<Body>`.
    pub fn build(self) -> Request<Body> {
        let mut request = self.inner.body(self.body).expect("request should build");
        // Merge stored extensions into the request.
        request.extensions_mut().extend(self.extensions);
        request
    }
}

// ---------------------------------------------------------------------------
// Oneshot helpers
// ---------------------------------------------------------------------------

/// Clone the app, send a single request via oneshot, and unwrap the result.
pub async fn send(app: &Router, req: Request<Body>) -> axum::response::Response {
    app.clone()
        .oneshot(req)
        .await
        .expect("oneshot should succeed")
}

/// Send a request, assert the status, and deserialize the JSON body.
pub async fn send_json<T: DeserializeOwned>(
    app: &Router,
    req: Request<Body>,
    status: StatusCode,
) -> T {
    let response = send(app, req).await;
    assert_json(response, status).await
}
