pub mod config;
pub mod providers;
pub mod routes;
pub mod state;

use anyhow::Context;
use axum::{middleware, Router};
use llm_tracing::{MemoryTraceCollector, TracingMiddleware};
use model_registry::Registry as ModelRegistry;
use smart_routing::{
    config::HealthConfig,
    executor::{ExecutorConfig, RouteExecutor},
    health::HealthManager,
    metrics::MetricsCollector,
    router::Router as SmartRouter,
    weight::AuthInfo,
};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use config::GatewayConfig;
use routes::{
    auth_middleware, chat_completions, health_check, list_models, rate_limit_middleware, root,
    route_request, security_headers_middleware,
};
use state::{AppState, DefaultRequestClassifier, RateLimiter, DEFAULT_RATE_LIMIT};

/// Build and run the gateway server.
pub async fn run() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "gateway=debug,tower_http=debug,axum=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load configuration
    let config = load_config()?;
    tracing::info!(
        "Loaded configuration with {} credentials",
        config.credentials.len()
    );

    // Warn if authentication is disabled
    if !config.is_auth_enabled() {
        tracing::warn!(
            "WARNING: No auth_tokens configured — authentication is DISABLED. \
             This is not recommended for production deployments."
        );
    }

    let state = build_app_state(config, None);

    // Get host and port from config before moving state
    let port = state.config.server.port;
    let host = &state.config.server.host;
    let addr: SocketAddr = format!("{}:{}", host, port)
        .parse()
        .context("Invalid host/port configuration")?;
    tracing::info!("Gateway listening on {}", addr);

    // Periodically prune expired rate-limit buckets to bound memory growth.
    let prune_limiter = Arc::clone(&state.rate_limiter);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(120));
        loop {
            interval.tick().await;
            prune_limiter.prune();
        }
    });

    let app = build_app_router(state);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    Ok(())
}

/// Load configuration from file or environment
pub fn load_config() -> anyhow::Result<GatewayConfig> {
    // Try to load from GATEWAY_CONFIG env var or default paths
    let config_path = std::env::var("GATEWAY_CONFIG").ok().or_else(|| {
        ["./gateway.yaml", "./config/gateway.yaml", "./gateway.yml"]
            .iter()
            .find(|path| std::path::Path::new(path).exists())
            .map(|path| path.to_string())
    });

    match config_path {
        Some(path) => {
            tracing::info!("Loading configuration from {}", path);
            match GatewayConfig::from_file(&path) {
                Ok(config) => Ok(config),
                Err(e) => {
                    anyhow::bail!(
                        "Failed to load config from {}: {}. Please fix the configuration file.",
                        path,
                        e
                    );
                },
            }
        },
        None => {
            tracing::info!("No configuration file found, using defaults");
            Ok(GatewayConfig::default())
        },
    }
}

/// Creates the application state from the given config.
///
/// Shared by [`run()`] and test helpers to ensure production and test setups
/// stay in sync. The `rate_limit` parameter overrides the default when
/// provided.
pub(crate) fn build_app_state(config: GatewayConfig, rate_limit: Option<u64>) -> AppState {
    let smart_router = config
        .credentials
        .iter()
        .fold(SmartRouter::new(), |mut router, cred| {
            router.add_credential(cred.id.clone(), cred.allowed_models.clone());
            router
        });

    let metrics = MetricsCollector::new();
    let health = HealthManager::new(HealthConfig::default());

    let executor = Arc::new(RouteExecutor::new(
        ExecutorConfig::default(),
        metrics,
        health,
    ));

    let classifier = Arc::new(DefaultRequestClassifier);

    let collector = Arc::new(MemoryTraceCollector::with_default_size());
    let tracing_middleware = TracingMiddleware::new(collector);

    let credentials: Vec<AuthInfo> = config
        .credentials
        .iter()
        .map(|c| AuthInfo {
            id: c.id.clone(),
            priority: Some(c.priority),
            quota_exceeded: false,
            unavailable: false,
            model_states: vec![],
        })
        .collect();

    let rate_limit = rate_limit.unwrap_or(DEFAULT_RATE_LIMIT);
    let rate_limiter = Arc::new(RateLimiter::new(rate_limit));

    AppState {
        config,
        registry: ModelRegistry::default(),
        router: smart_router,
        executor,
        classifier,
        tracing: tracing_middleware,
        start_time: Instant::now(),
        credentials,
        rate_limiter,
    }
}

/// Constructs the complete Axum router with all middleware layers in
/// production order.
///
/// Shared by [`run()`] and test helpers to guarantee the router structure
/// never diverges between production and test builds.
pub(crate) fn build_app_router(state: AppState) -> Router {
    let public_routes = Router::new()
        .route("/", axum::routing::get(root))
        .route("/health", axum::routing::get(health_check));

    let protected_routes = Router::new()
        .route("/api/models", axum::routing::get(list_models))
        .route("/api/route", axum::routing::get(route_request))
        .route(
            "/v1/chat/completions",
            axum::routing::post(chat_completions),
        )
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            rate_limit_middleware,
        ))
        .layer(middleware::from_fn(security_headers_middleware))
        .layer(middleware::from_fn_with_state(
            state.tracing.clone(),
            llm_tracing::tracing_middleware,
        ))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

#[cfg(test)]
#[allow(dead_code)]
mod test_helpers {
    use super::*;
    use config::CredentialConfig;

    /// Configurable fields for customizing test application state.
    /// Fields set to `None` use sensible defaults.
    pub(crate) struct TestOverrides {
        pub auth_tokens: Vec<String>,
        pub credentials: Vec<CredentialConfig>,
        pub rate_limit: Option<u64>,
    }

    impl Default for TestOverrides {
        fn default() -> Self {
            Self {
                auth_tokens: vec!["test-token".to_string()],
                credentials: vec![],
                rate_limit: None,
            }
        }
    }

    /// Builds an [`AppState`] from the given overrides, using sensible defaults
    /// for any field not provided.
    pub(crate) fn create_test_state(overrides: TestOverrides) -> AppState {
        let mut config = GatewayConfig::default();
        config.server.auth_tokens = overrides.auth_tokens;
        config.credentials = overrides.credentials;
        build_app_state(config, overrides.rate_limit)
    }

    /// Constructs the complete Axum router with all middleware layers in
    /// production order. Returns a [`Router`] for use with
    /// [`tower::ServiceExt::oneshot()`].
    ///
    /// Insert [`ConnectInfo<SocketAddr>`] into each test request's extensions
    /// to satisfy the rate limiter's address extractor.
    pub(crate) fn build_full_app(state: AppState) -> Router {
        build_app_router(state)
    }
}
