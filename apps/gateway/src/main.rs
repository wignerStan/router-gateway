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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
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

    // Initialize model registry
    let registry = ModelRegistry::default();
    tracing::info!("Model registry initialized");

    // Initialize smart router and populate from config
    let smart_router = config
        .credentials
        .iter()
        .fold(SmartRouter::new(), |mut router, cred| {
            router.add_credential(cred.id.clone(), cred.allowed_models.clone());
            router
        });

    tracing::info!(
        "Smart router initialized with {} credentials",
        config.credentials.len()
    );

    // Initialize metrics and health for executor
    let metrics = MetricsCollector::new();
    let health = HealthManager::new(HealthConfig::default());

    // Initialize route executor
    let executor_config = ExecutorConfig::default();
    let executor = Arc::new(RouteExecutor::new(executor_config, metrics, health));
    tracing::info!("Route executor initialized");

    // Initialize request classifier
    let classifier = Arc::new(DefaultRequestClassifier);

    // Initialize tracing middleware
    let collector = Arc::new(MemoryTraceCollector::with_default_size());
    let tracing_middleware = TracingMiddleware::new(collector);
    tracing::info!("LLM tracing initialized");

    // Convert credentials to AuthInfo for routing
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

    // Initialize rate limiter
    let rate_limiter = Arc::new(RateLimiter::new(DEFAULT_RATE_LIMIT));
    tracing::info!(
        "Rate limiter initialized: {} requests per minute per IP",
        DEFAULT_RATE_LIMIT
    );

    // Periodically prune expired rate-limit buckets to bound memory growth.
    let prune_limiter = Arc::clone(&rate_limiter);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(120));
        loop {
            interval.tick().await;
            prune_limiter.prune();
        }
    });

    let state = AppState {
        config,
        registry,
        router: smart_router,
        executor,
        classifier,
        tracing: tracing_middleware.clone(),
        start_time: Instant::now(),
        credentials,
        rate_limiter,
    };

    // Get host and port from config before moving state
    let port = state.config.server.port;
    let host = &state.config.server.host;
    let addr: SocketAddr = format!("{}:{}", host, port)
        .parse()
        .context("Invalid host/port configuration")?;
    tracing::info!("Gateway listening on {}", addr);

    // Build our application with routes
    // Public routes (no authentication required)
    let public_routes = Router::new()
        .route("/", axum::routing::get(root))
        .route("/health", axum::routing::get(health_check));

    // Protected routes (require authentication if auth_tokens configured)
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

    let app = Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            rate_limit_middleware,
        ))
        .layer(middleware::from_fn(security_headers_middleware))
        .layer(middleware::from_fn_with_state(
            tracing_middleware,
            llm_tracing::tracing_middleware,
        ))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    Ok(())
}

/// Load configuration from file or environment
fn load_config() -> anyhow::Result<GatewayConfig> {
    // Try to load from GATEWAY_CONFIG env var or default paths
    let config_path = std::env::var("GATEWAY_CONFIG").ok().or_else(|| {
        // Check for common config file locations
        for path in ["./gateway.yaml", "./config/gateway.yaml", "./gateway.yml"] {
            if std::path::Path::new(path).exists() {
                return Some(path.to_string());
            }
        }
        None
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
