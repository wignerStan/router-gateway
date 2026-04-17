//! HTTP input validation and edge-case tests for route handlers.
//!
//! Covers malformed JSON bodies, wrong content-types, missing fields,
//! unsupported HTTP methods, not-found endpoints, empty objects, and
//! oversized inputs.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode};
use gateway::config::GatewayConfig;
use gateway::{build_app_router, build_app_state};
use serde_json::json;

mod red_edge {
    use super::*;

    /// Helper: create a fully-wired app router with one auth token so protected
    /// routes return meaningful handler errors instead of 403.
    fn app_with_auth() -> Router {
        let mut config = GatewayConfig::default();
        config.server.auth_tokens = vec!["test-token".to_string()];
        build_app_router(build_app_state(config, None))
    }

    /// Helper: create an app with default config (no auth tokens).
    fn app_no_auth() -> Router {
        let config = GatewayConfig::default();
        build_app_router(build_app_state(config, None))
    }

    /// Build a raw request and inject `ConnectInfo<SocketAddr>` so that the
    /// rate-limit middleware (which requires it) does not panic.
    fn raw_request_with_connect_info(request: Request<Body>) -> Request<Body> {
        let mut req = request;
        req.extensions_mut()
            .insert(ConnectInfo(common::test_addr()));
        req
    }

    // ---------------------------------------------------------------------------
    // Malformed / invalid JSON bodies
    // ---------------------------------------------------------------------------

    #[tokio::test]
    async fn malformed_json_returns_unprocessable_entity() {
        let app = app_with_auth();

        let request = raw_request_with_connect_info(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("content-type", "application/json")
                .header("authorization", "Bearer test-token")
                .body(Body::from("{invalid json"))
                .expect("request should build"),
        );

        let response = common::send(&app, request).await;
        let status = response.status();

        // Axum's Json extractor returns 400 Bad Request for malformed JSON.
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn empty_body_returns_bad_request() {
        let app = app_with_auth();

        let request = raw_request_with_connect_info(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("content-type", "application/json")
                .header("authorization", "Bearer test-token")
                .body(Body::empty())
                .expect("request should build"),
        );

        let response = common::send(&app, request).await;
        let status = response.status();

        // Axum's Json extractor returns 400 Bad Request for an empty body when
        // JSON is expected.
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn wrong_content_type_returns_unsupported_media_type() {
        let app = app_with_auth();

        let body = json!({"model": "gpt-4", "messages": [{"role": "user", "content": "hi"}]});
        let body_bytes = serde_json::to_vec(&body).unwrap();

        let request = raw_request_with_connect_info(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("content-type", "text/plain")
                .header("authorization", "Bearer test-token")
                .body(Body::from(body_bytes))
                .expect("request should build"),
        );

        let response = common::send(&app, request).await;
        let status = response.status();

        // Axum rejects the request with 415 Unsupported Media Type when the
        // content-type does not match the Json extractor's expected MIME type.
        assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }

    // ---------------------------------------------------------------------------
    // Semantically invalid JSON payloads
    // ---------------------------------------------------------------------------

    #[tokio::test]
    async fn missing_model_field_returns_service_unavailable() {
        let app = app_with_auth();

        let body = json!({"messages": [{"role": "user", "content": "hi"}]});
        let request = common::RequestBuilder::post_json("/v1/chat/completions", &body)
            .with_auth("test-token")
            .with_connect_info(common::test_addr())
            .build();

        let response = common::send(&app, request).await;
        let status = response.status();

        // The handler treats missing model as "unknown" and proceeds to routing,
        // which fails with 503 because no credentials are configured.
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn null_messages_returns_service_unavailable() {
        let app = app_with_auth();

        let body = json!({"model": "gpt-4", "messages": null});
        let request = common::RequestBuilder::post_json("/v1/chat/completions", &body)
            .with_auth("test-token")
            .with_connect_info(common::test_addr())
            .build();

        let response = common::send(&app, request).await;
        let status = response.status();

        // The handler classifies the request and routes it; null messages still
        // produces a classification. With no credentials configured, this
        // results in 503.
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn empty_json_object_returns_service_unavailable() {
        let app = app_with_auth();

        let body = json!({});
        let request = common::RequestBuilder::post_json("/v1/chat/completions", &body)
            .with_auth("test-token")
            .with_connect_info(common::test_addr())
            .build();

        let response = common::send(&app, request).await;
        let status = response.status();

        // Empty object: no model, no messages. Handler defaults model to "unknown"
        // and routes — no credentials means 503.
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn extra_large_model_name_returns_service_unavailable() {
        let app = app_with_auth();

        let long_name: String = "x".repeat(10_000);
        let body = json!({"model": long_name, "messages": [{"role": "user", "content": "hi"}]});
        let request = common::RequestBuilder::post_json("/v1/chat/completions", &body)
            .with_auth("test-token")
            .with_connect_info(common::test_addr())
            .build();

        let response = common::send(&app, request).await;
        let status = response.status();

        // The handler accepts any model string and routes it. With no credentials
        // configured, this results in 503.
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    }

    // ---------------------------------------------------------------------------
    // Method not allowed
    // ---------------------------------------------------------------------------

    #[tokio::test]
    async fn delete_method_not_allowed() {
        let app = app_with_auth();

        let request = raw_request_with_connect_info(
            Request::builder()
                .method("DELETE")
                .uri("/v1/chat/completions")
                .header("authorization", "Bearer test-token")
                .body(Body::empty())
                .expect("request should build"),
        );

        let response = common::send(&app, request).await;
        let status = response.status();

        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
    }

    #[tokio::test]
    async fn put_method_not_allowed() {
        let app = app_with_auth();

        let request = raw_request_with_connect_info(
            Request::builder()
                .method("PUT")
                .uri("/v1/chat/completions")
                .header("authorization", "Bearer test-token")
                .body(Body::empty())
                .expect("request should build"),
        );

        let response = common::send(&app, request).await;
        let status = response.status();

        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
    }

    // ---------------------------------------------------------------------------
    // Not found
    // ---------------------------------------------------------------------------

    #[tokio::test]
    async fn not_found_get_nonexistent() {
        let app = app_no_auth();

        let request = common::RequestBuilder::get("/api/nonexistent")
            .with_connect_info(common::test_addr())
            .build();

        let response = common::send(&app, request).await;
        let status = response.status();

        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn not_found_post_nonexistent() {
        let app = app_with_auth();

        let request = common::RequestBuilder::post_json("/v1/nonexistent", &json!({}))
            .with_auth("test-token")
            .with_connect_info(common::test_addr())
            .build();

        let response = common::send(&app, request).await;
        let status = response.status();

        assert_eq!(status, StatusCode::NOT_FOUND);
    }
}
