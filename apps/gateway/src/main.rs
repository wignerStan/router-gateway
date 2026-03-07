use axum::{extract::State, routing::get, Json, Router};
use serde_json::json;
use std::net::SocketAddr;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// Local package imports
use llm_tracing::{MemoryTraceCollector, TracingMiddleware};
use model_registry::Registry as ModelRegistry;
use smart_routing::Router as SmartRouter;
use std::sync::Arc;

#[derive(Clone)]
#[allow(dead_code)] // Fields will be used when routing logic is implemented
struct AppState {
    registry: ModelRegistry,
    router: SmartRouter,
    tracing: TracingMiddleware,
}

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

    // Initialize model registry
    let registry = ModelRegistry::default();
    tracing::info!("Model registry initialized");

    // Initialize smart router
    let smart_router = SmartRouter::new();
    tracing::info!("Smart router initialized");

    // Initialize tracing middleware
    let collector = Arc::new(MemoryTraceCollector::with_default_size());
    let tracing_middleware = TracingMiddleware::new(collector);
    tracing::info!("LLM tracing initialized");

    let state = AppState {
        registry,
        router: smart_router,
        tracing: tracing_middleware.clone(),
    };

    // Build our application with routes
    let app = Router::new()
        .route("/", get(root))
        .route("/health", get(health_check))
        .route("/api/models", get(list_models))
        .route("/api/route", get(route_request))
        .layer(axum::middleware::from_fn_with_state(
            tracing_middleware,
            llm_tracing::tracing_middleware,
        ))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    // Create a TCP listener
    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::info!("Gateway listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn root() -> Json<serde_json::Value> {
    Json(json!({
        "name": "Gateway API",
        "version": "0.1.0",
        "description": "Smart routing gateway for LLM requests",
        "features": [
            "Smart Routing",
            "Model Registry",
            "LLM Tracing"
        ]
    }))
}

async fn health_check() -> &'static str {
    "OK"
}

async fn list_models(State(_state): State<AppState>) -> Json<serde_json::Value> {
    // TODO: Implement actual model listing from registry
    Json(json!({
        "models": [],
        "count": 0,
        "message": "Model registry integration pending"
    }))
}

async fn route_request(State(_state): State<AppState>) -> Json<serde_json::Value> {
    // Example routing logic
    Json(json!({
        "routed_to": "example-model",
        "status": "success",
        "message": "Smart routing integration pending"
    }))
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_health_endpoint() {
        let app = Router::new().route("/health", get(health_check));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_root_endpoint() {
        let app = Router::new().route("/", get(root));

        let response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_basic_routing() {
        let app = Router::new()
            .route("/", get(root))
            .route("/health", get(health_check));

        // Test root path
        let response = app
            .clone()
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        // Test health path
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}
