//! Shared e2e test utilities for container-based testing.
//!
//! This module provides helpers for starting PostgreSQL and Redis containers
//! via testcontainers-rs 0.25.0, building connection strings with dynamic
//! port assignment, and managing test timeouts.
//!
//! # Podman Compatibility
//!
//! Set `DOCKER_HOST=unix:///run/podman/podman.sock` before running e2e tests
//! to use Podman as the container runtime instead of Docker.
//!
//! # Feature Gate
//!
//! This module is gated behind `#[cfg(feature = "e2e")]` at the inclusion site
//! and will not be compiled during normal `cargo test` runs.
//!
//! # Container Lifecycle
//!
//! Containers auto-cleanup when their handle (`ContainerAsync`) is dropped.
//! No manual teardown is needed. The resource reaper (Ryuk) provides additional
//! cleanup guarantees for CI environments. To disable Ryuk in CI, set
//! `TESTCONTAINERS_RYUK_DISABLED=true`.

#![allow(missing_docs, clippy::expect_used, clippy::panic)]

use testcontainers::{runners::AsyncRunner, ContainerAsync};
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::redis::Redis;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default timeout for container startup operations (seconds).
pub const CONTAINER_STARTUP_TIMEOUT_SECS: u64 = 30;

/// Default timeout for individual test operations (seconds).
pub const OPERATION_TIMEOUT_SECS: u64 = 10;

/// PostgreSQL internal port inside the container.
pub const POSTGRES_PORT: u16 = 5432;

/// Redis internal port inside the container.
pub const REDIS_PORT: u16 = 6379;

/// Default PostgreSQL superuser name (set by testcontainers-modules).
pub const POSTGRES_USER: &str = "postgres";

/// Default PostgreSQL superuser password (set by testcontainers-modules).
pub const POSTGRES_PASSWORD: &str = "postgres";

/// Default PostgreSQL database name (set by testcontainers-modules).
pub const POSTGRES_DB: &str = "postgres";

// ---------------------------------------------------------------------------
// Container Startup Helpers
// ---------------------------------------------------------------------------

/// Start a PostgreSQL container using the default image from testcontainers-modules.
///
/// The container uses default credentials (`postgres`/`postgres`) and the
/// default database (`postgres`). It exposes port [`POSTGRES_PORT`] and
/// auto-cleans when the returned handle is dropped.
///
/// # Panics
///
/// Panics if the container runtime is unavailable or the container fails to
/// start within the default timeout.
///
/// # Example
///
/// ```rust,ignore
/// let pg = common::start_postgres().await;
/// let url = common::format_postgres_url(&pg).await;
/// // Use `url` to connect to the real PostgreSQL instance.
/// // Container auto-cleans when `pg` is dropped.
/// ```
pub async fn start_postgres() -> ContainerAsync<Postgres> {
    Postgres::default()
        .start()
        .await
        .expect("PostgreSQL container should start within the default timeout")
}

/// Start a Redis container using the default image from testcontainers-modules.
///
/// The container starts with no authentication (default for testcontainers-modules).
/// It exposes port [`REDIS_PORT`] and auto-cleans when the returned handle is
/// dropped.
///
/// # Panics
///
/// Panics if the container runtime is unavailable or the container fails to
/// start within the default timeout.
///
/// # Example
///
/// ```rust,ignore
/// let redis = common::start_redis().await;
/// let url = common::format_redis_url(&redis).await;
/// // Use `url` to connect to the real Redis instance.
/// // Container auto-cleans when `redis` is dropped.
/// ```
pub async fn start_redis() -> ContainerAsync<Redis> {
    Redis::default()
        .start()
        .await
        .expect("Redis container should start within the default timeout")
}

// ---------------------------------------------------------------------------
// Connection String Builders
// ---------------------------------------------------------------------------

/// Build a PostgreSQL connection string using the container's dynamically
/// assigned host port.
///
/// Uses `127.0.0.1` as the host, which is appropriate for local Docker/Podman.
/// Credentials and database name are the testcontainers-modules defaults.
///
/// # Panics
///
/// Panics if the port mapping is not available (container not yet started).
///
/// # Example
///
/// ```rust,ignore
/// let pg = common::start_postgres().await;
/// let url = common::format_postgres_url(&pg).await;
/// assert!(url.starts_with("postgres://postgres:postgres@127.0.0.1:"));
/// ```
pub async fn format_postgres_url(container: &ContainerAsync<Postgres>) -> String {
    let port = container
        .get_host_port_ipv4(POSTGRES_PORT)
        .await
        .expect("PostgreSQL port mapping should be available after container starts");
    format!("postgres://{POSTGRES_USER}:{POSTGRES_PASSWORD}@127.0.0.1:{port}/{POSTGRES_DB}")
}

/// Build a Redis connection string using the container's dynamically
/// assigned host port.
///
/// Uses `127.0.0.1` as the host, which is appropriate for local Docker/Podman.
///
/// # Panics
///
/// Panics if the port mapping is not available (container not yet started).
///
/// # Example
///
/// ```rust,ignore
/// let redis = common::start_redis().await;
/// let url = common::format_redis_url(&redis).await;
/// assert!(url.starts_with("redis://127.0.0.1:"));
/// ```
pub async fn format_redis_url(container: &ContainerAsync<Redis>) -> String {
    let port = container
        .get_host_port_ipv4(REDIS_PORT)
        .await
        .expect("Redis port mapping should be available after container starts");
    format!("redis://127.0.0.1:{port}")
}
