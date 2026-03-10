use axum::{extract::State, routing::get, Json, Router};
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// Local package imports
use llm_tracing::{MemoryTraceCollector, TracingMiddleware};
use model_registry::Registry as ModelRegistry;
use smart_routing::{
    classification::{
        detection::ToolDetector, ContentTypeDetector, FormatDetector, RequestClassifier,
        StreamingExtractor, TokenEstimator,
    },
    config::HealthConfig,
    executor::{ExecutorConfig, RouteExecutor},
    health::HealthManager,
    metrics::MetricsCollector,
    router::Router as SmartRouter,
};

/// Default request classifier implementation
struct DefaultRequestClassifier;

impl RequestClassifier for DefaultRequestClassifier {
    fn classify(&self, request: &Value) -> smart_routing::classification::ClassifiedRequest {
        use smart_routing::classification::{
            ClassifiedRequest, QualityPreference, RequiredCapabilities,
        };

        // Detect required capabilities
        let vision = ContentTypeDetector::detect_vision_required(request);
        let tools = ToolDetector::detect_tools_required(request);
        let streaming = StreamingExtractor::extract_streaming_preference(request);
        let thinking = false; // Could be enhanced with reasoning detection

        // Detect format
        let format = FormatDetector::detect(request);

        // Estimate tokens
        let estimated_tokens = TokenEstimator::estimate(request);

        // Determine quality preference (could be enhanced with request analysis)
        let quality_preference = QualityPreference::Balanced;

        ClassifiedRequest {
            required_capabilities: RequiredCapabilities {
                vision,
                tools,
                streaming,
                thinking,
            },
            estimated_tokens,
            format,
            quality_preference,
        }
    }
}

struct AppState {
    registry: ModelRegistry,
    router: SmartRouter,
    executor: Arc<RouteExecutor>,
    classifier: Arc<DefaultRequestClassifier>,
    tracing: TracingMiddleware,
}

impl Clone for AppState {
    fn clone(&self) -> Self {
        Self {
            registry: self.registry.clone(),
            router: self.router.clone(),
            executor: Arc::clone(&self.executor),
            classifier: Arc::clone(&self.classifier),
            tracing: self.tracing.clone(),
        }
    }
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

    let state = AppState {
        registry,
        router: smart_router,
        executor,
        classifier,
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

async fn route_request(State(state): State<AppState>) -> Json<serde_json::Value> {
    // Create a sample request for demonstration
    // In production, this would come from the request body as JSON
    let sample_request = json!({
        "messages": [
            {
                "role": "user",
                "content": "Hello, how are you?"
            }
        ],
        "model": "unknown"
    });

    // Step 1: Classify the request using RequestClassifier
    let classified = state.classifier.classify(&sample_request);

    // Step 2: Plan routes using Router
    // Note: In production, auths would be loaded from configuration/database
    let auths: Vec<smart_routing::weight::AuthInfo> = vec![];
    let session_id: Option<&str> = None;

    let route_plan = state.router.plan(&classified, auths, session_id).await;

    // Step 3: Return the route plan
    // In production, Step 4 would execute the route using RouteExecutor
    // and Step 5 would return the LLM response

    // Format the primary route
    let primary_json = match &route_plan.primary {
        Some(primary) => json!({
            "credential_id": primary.credential_id,
            "model_id": primary.model_id,
            "provider": primary.provider,
            "utility": primary.utility,
            "weight": primary.weight,
        }),
        None => json!(null),
    };

    // Format fallbacks
    let fallbacks_json: Vec<serde_json::Value> = route_plan
        .fallbacks
        .iter()
        .map(|fb| {
            json!({
                "credential_id": fb.auth_id,
                "position": fb.position,
                "weight": fb.weight,
                "provider": fb.provider,
            })
        })
        .collect();

    // Build response
    let response = json!({
        "route_plan": {
            "primary": primary_json,
            "fallbacks": fallbacks_json,
            "total_candidates": route_plan.total_candidates,
            "filtered_candidates": route_plan.filtered_candidates,
        },
        "classification": {
            "required_capabilities": {
                "vision": classified.required_capabilities.vision,
                "tools": classified.required_capabilities.tools,
                "streaming": classified.required_capabilities.streaming,
                "thinking": classified.required_capabilities.thinking
            },
            "estimated_tokens": classified.estimated_tokens,
            "format": format!("{:?}", classified.format),
            "quality_preference": format!("{:?}", classified.quality_preference)
        },
        "message": if route_plan.primary.is_some() {
            "Route planned successfully"
        } else {
            "No suitable routes found - credentials need to be configured"
        }
    });

    Json(response)
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
