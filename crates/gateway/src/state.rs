//! Application state types for the gateway

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::config::GatewayConfig;
use llm_tracing::TracingMiddleware;
use model_registry::Registry as ModelRegistry;
use smart_routing::executor::RouteExecutor;
use smart_routing::{
    classification::{
        ContentTypeDetector, FormatDetector, RequestClassifier, StreamingExtractor, ToolDetector,
    },
    router::Router as SmartRouter,
    weight::AuthInfo,
};

/// Default rate limit: requests per minute per client IP.
pub const DEFAULT_RATE_LIMIT: u64 = 60;

/// Application state shared across handlers
pub struct AppState {
    /// Gateway configuration
    pub config: GatewayConfig,
    /// Model registry for model information
    pub registry: ModelRegistry,
    /// Smart router for route planning
    pub router: SmartRouter,
    /// Route executor for running requests
    pub executor: Arc<RouteExecutor>,
    /// Request classifier
    pub classifier: Arc<DefaultRequestClassifier>,
    /// Tracing middleware
    pub tracing: TracingMiddleware,
    /// Server start time for uptime tracking
    pub start_time: Instant,
    /// Credential information for routing
    pub credentials: Vec<AuthInfo>,
    /// Rate limiter for per-IP request throttling
    pub rate_limiter: Arc<RateLimiter>,
}

impl Clone for AppState {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            registry: self.registry.clone(),
            router: self.router.clone(),
            executor: Arc::clone(&self.executor),
            classifier: Arc::clone(&self.classifier),
            tracing: self.tracing.clone(),
            start_time: self.start_time,
            credentials: self.credentials.clone(),
            rate_limiter: Arc::clone(&self.rate_limiter),
        }
    }
}

/// Default request classifier implementation
pub struct DefaultRequestClassifier;

impl RequestClassifier for DefaultRequestClassifier {
    fn classify(
        &self,
        request: &serde_json::Value,
    ) -> smart_routing::classification::ClassifiedRequest {
        use smart_routing::classification::{
            ClassifiedRequest, QualityPreference, RequiredCapabilities,
        };

        let vision = ContentTypeDetector::detect_vision_required(request);
        let tools = ToolDetector::detect_tools_required(request);
        let streaming = StreamingExtractor::extract_streaming_preference(request);
        let thinking = false;

        let format = FormatDetector::detect(request);
        let estimated_tokens = smart_routing::classification::TokenEstimator::estimate(request);
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

/// Health status response
#[derive(Debug, Serialize, Deserialize)]
pub struct HealthStatus {
    pub status: String,
    pub uptime_secs: u64,
    pub credential_count: usize,
    pub healthy_count: usize,
    pub degraded_count: usize,
    pub unhealthy_count: usize,
}

/// Model information for API response
#[derive(Debug, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub provider: String,
    pub capabilities: Vec<String>,
    pub context_window: usize,
}

/// In-memory rate limiter tracking request counts per client IP
/// within a sliding one-minute window.
pub struct RateLimiter {
    /// IP address -> (request count, window start time)
    buckets: Arc<Mutex<HashMap<String, (u64, Instant)>>>,
    max_requests: u64,
}

impl RateLimiter {
    pub fn new(max_requests: u64) -> Self {
        Self {
            buckets: Arc::new(Mutex::new(HashMap::new())),
            max_requests,
        }
    }

    /// Check whether a request from the given IP should be allowed.
    /// Returns `true` if under the limit, `false` if rate limited.
    pub fn check(&self, ip: &str) -> bool {
        let mut buckets = self
            .buckets
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let now = Instant::now();

        let (count, window_start) = buckets.entry(ip.to_string()).or_insert((0, now));

        // Reset window if more than 60 seconds have passed
        if now.duration_since(*window_start).as_secs() >= 60 {
            *count = 0;
            *window_start = now;
        }

        if *count >= self.max_requests {
            return false;
        }

        *count += 1;
        true
    }

    /// Remove expired rate-limit entries to bound memory growth.
    /// Called periodically from a background task.
    pub(crate) fn prune(&self) {
        // ALLOW: Mutex poisoning is an acceptable panic — propagates failure for
        // inconsistent shared state.
        #[allow(clippy::expect_used)]
        let mut buckets = self.buckets.lock().expect("Rate limiter mutex poisoned");
        let now = Instant::now();
        buckets.retain(|_, (_, window_start)| now.duration_since(*window_start).as_secs() < 120);
    }
}
