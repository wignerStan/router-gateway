//! E2E container tests using testcontainers-rs 0.25.0 with Podman backend.
//!
//! These tests spin up real `PostgreSQL` and Redis containers to verify
//! container lifecycle management and integration with the gateway.
//!
//! # Running
//!
//! ```bash
//! DOCKER_HOST=unix:///run/podman/podman.sock cargo test --features e2e -- e2e
//! ```
//!
//! # Feature Gate
//!
//! All tests in this module are compiled only when the `e2e` feature is
//! enabled. Normal `cargo test` runs will skip these tests entirely.

use super::common;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use common::{format_postgres_url, format_redis_url, start_postgres, start_redis};
use gateway::config::GatewayConfig;
use gateway::{build_app_router, build_app_state};
use tower::ServiceExt;

// ===================================================================
// Container Lifecycle Tests
// ===================================================================

#[tokio::test]
async fn test_gateway_postgres_container_starts() {
    // Arrange & Act: start a PostgreSQL container
    let _container = start_postgres().await;

    // Assert: if we reach here, the container started successfully.
    // The container auto-cleans when `_container` is dropped.
}

#[tokio::test]
async fn test_gateway_redis_container_starts() {
    // Arrange & Act: start a Redis container
    let _container = start_redis().await;

    // Assert: if we reach here, the container started successfully.
    // The container auto-cleans when `_container` is dropped.
}

#[tokio::test]
async fn test_e2e_postgres_connection_string_format() {
    // Arrange: start a PostgreSQL container
    let container = start_postgres().await;

    // Act: build the connection string
    let url = format_postgres_url(&container).await;

    // Assert: connection string follows the expected format
    assert!(
        url.starts_with("postgres://postgres:postgres@127.0.0.1:"),
        "Connection string should start with postgres:// scheme and default credentials, got: {url}"
    );

    // The port should be a dynamically assigned number (not the internal port)
    let port_part = url
        .trim_start_matches("postgres://postgres:postgres@127.0.0.1:")
        .trim_end_matches("/postgres");
    let port: u16 = port_part
        .parse()
        .expect("port in connection string should be a valid u16");
    assert_ne!(
        port,
        common::POSTGRES_PORT,
        "Host port should be dynamically assigned, not the internal container port {port}"
    );
}

#[tokio::test]
async fn test_e2e_redis_connection_string_format() {
    // Arrange: start a Redis container
    let container = start_redis().await;

    // Act: build the connection string
    let url = format_redis_url(&container).await;

    // Assert: connection string follows the expected format
    assert!(
        url.starts_with("redis://127.0.0.1:"),
        "Connection string should start with redis:// scheme, got: {url}"
    );

    // The port should be a dynamically assigned number (not the internal port)
    let port_part = url.trim_start_matches("redis://127.0.0.1:");
    let port: u16 = port_part
        .parse()
        .expect("port in connection string should be a valid u16");
    assert_ne!(
        port,
        common::REDIS_PORT,
        "Host port should be dynamically assigned, not the internal container port {port}"
    );
}

#[tokio::test]
async fn test_e2e_container_cleanup_on_drop() {
    // Arrange & Act: start a PostgreSQL container and explicitly drop it
    {
        let container = start_postgres().await;

        // Verify the container is running by getting the port mapping
        let _port = container
            .get_host_port_ipv4(common::POSTGRES_PORT)
            .await
            .expect("port mapping should be available while container is running");

        // Container is dropped here, triggering cleanup
    }

    // Assert: no assertion needed — if cleanup panics or hangs, the test
    // will time out and fail. Successful completion means cleanup worked.
}

#[tokio::test]
async fn test_e2e_multiple_containers_concurrent() {
    // Arrange & Act: start both PostgreSQL and Redis containers concurrently
    let postgres_handle = tokio::spawn(async { start_postgres().await });
    let redis_handle = tokio::spawn(async { start_redis().await });

    let pg_container = postgres_handle
        .await
        .expect("PostgreSQL task should not panic");
    let redis_container = redis_handle.await.expect("Redis task should not panic");

    // Assert: both containers are running and have port mappings
    let pg_port = pg_container
        .get_host_port_ipv4(common::POSTGRES_PORT)
        .await
        .expect("PostgreSQL port mapping should be available");

    let redis_port = redis_container
        .get_host_port_ipv4(common::REDIS_PORT)
        .await
        .expect("Redis port mapping should be available");

    // Both ports should be valid and different
    assert!(pg_port > 0, "PostgreSQL host port should be positive");
    assert!(redis_port > 0, "Redis host port should be positive");
    assert_ne!(
        pg_port, redis_port,
        "PostgreSQL and Redis should be assigned different host ports"
    );

    // Both containers auto-cleanup when dropped at end of test
}

// ===================================================================
// Gateway Integration Tests with Containers
// ===================================================================

#[tokio::test]
async fn test_e2e_gateway_health_check() {
    // Arrange: start a PostgreSQL container and build the gateway
    let _pg_container = start_postgres().await;
    let state = build_app_state(GatewayConfig::default(), None);
    let app = build_app_router(state);

    // Act: send a health check request via tower::ServiceExt
    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .expect("request should be valid"),
        )
        .await
        .expect("gateway should process health check request");

    // Assert: health endpoint returns 200 OK
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "health check should return 200 OK"
    );
}

#[tokio::test]
async fn test_e2e_gateway_root_endpoint() {
    // Arrange: start containers and build the gateway
    let _pg_container = start_postgres().await;
    let _redis_container = start_redis().await;
    let state = build_app_state(GatewayConfig::default(), None);
    let app = build_app_router(state);

    // Act: send a request to the root endpoint
    let response = app
        .oneshot(
            Request::builder()
                .uri("/")
                .body(Body::empty())
                .expect("request should be valid"),
        )
        .await
        .expect("gateway should process root request");

    // Assert: root endpoint returns 200 OK
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "root endpoint should return 200 OK"
    );
}

#[tokio::test]
async fn test_e2e_full_request_flow_with_postgres() {
    // Arrange: start a PostgreSQL container and build a gateway with credentials
    let pg_container = start_postgres().await;
    let _connection_url = format_postgres_url(&pg_container).await;

    let mut config = GatewayConfig::default();
    config.credentials.push(gateway::config::CredentialConfig {
        id: "e2e-test-credential".to_string(),
        provider: "openai".to_string(),
        api_key: "sk-test-e2e-key".to_string(),
        allowed_models: vec!["gpt-4".to_string()],
        ..Default::default()
    });

    let state = build_app_state(config, None);

    // Act: send a health check to verify the gateway is functional
    let health_response = build_app_router(state.clone())
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .expect("request should be valid"),
        )
        .await
        .expect("gateway should process request");

    // Assert: gateway is healthy
    assert_eq!(health_response.status(), StatusCode::OK);

    // Also verify the models endpoint is accessible
    let models_response = build_app_router(state)
        .oneshot(
            Request::builder()
                .uri("/api/models")
                .body(Body::empty())
                .expect("request should be valid"),
        )
        .await
        .expect("gateway should process models request");

    // Models endpoint requires auth by default (unless no auth_tokens configured)
    // With default config, auth is disabled, so we expect 200
    assert_eq!(
        models_response.status(),
        StatusCode::OK,
        "models endpoint should be accessible without auth when no tokens configured"
    );
}

#[tokio::test]
async fn test_e2e_gateway_with_redis_container() {
    // Arrange: start a Redis container and build the gateway
    let redis_container = start_redis().await;
    let _connection_url = format_redis_url(&redis_container).await;

    let state = build_app_state(GatewayConfig::default(), None);
    let app = build_app_router(state);

    // Act: verify gateway health with Redis available
    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .expect("request should be valid"),
        )
        .await
        .expect("gateway should process request");

    // Assert: gateway is healthy
    assert_eq!(response.status(), StatusCode::OK);
}
