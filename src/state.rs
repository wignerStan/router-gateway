//! Application state types for the gateway

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::config::GatewayConfig;
use crate::registry::Registry as ModelRegistry;
use crate::routing::executor::RouteExecutor;
use crate::routing::{
    classification::{
        ContentTypeDetector, FormatDetector, RequestClassifier, StreamingExtractor, ToolDetector,
    },
    router::Router as SmartRouter,
    weight::AuthInfo,
};
use crate::tracing::TracingMiddleware;

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
    ) -> crate::routing::classification::ClassifiedRequest {
        use crate::routing::classification::{
            ClassifiedRequest, QualityPreference, RequiredCapabilities,
        };

        let vision = ContentTypeDetector::detect_vision_required(request);
        let tools = ToolDetector::detect_tools_required(request);
        let streaming = StreamingExtractor::extract_streaming_preference(request);
        let thinking = false;

        let format = FormatDetector::detect(request);
        let estimated_tokens = crate::routing::classification::TokenEstimator::estimate(request);
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
    /// Overall health status string.
    pub status: String,
    /// Seconds since the gateway started.
    pub uptime_secs: u64,
    /// Total number of configured credentials.
    pub credential_count: usize,
    /// Number of credentials reporting healthy status.
    pub healthy_count: usize,
    /// Number of credentials reporting degraded status.
    pub degraded_count: usize,
    /// Number of credentials reporting unhealthy status.
    pub unhealthy_count: usize,
}

/// Model information for API response
#[derive(Debug, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Unique model identifier.
    pub id: String,
    /// Provider name (e.g. "openai", "google").
    pub provider: String,
    /// List of capability tags for this model.
    pub capabilities: Vec<String>,
    /// Maximum context window size in tokens.
    pub context_window: usize,
}

/// In-memory rate limiter tracking request counts per client IP
/// within a sliding one-minute window.
pub struct RateLimiter {
    /// IP address -> (request count, window start time)
    buckets: Arc<Mutex<HashMap<String, (u64, Instant)>>>,
    /// Maximum requests allowed per window.
    max_requests: u64,
}

impl RateLimiter {
    /// Create a new rate limiter with the given maximum requests per window.
    #[must_use]
    pub fn new(max_requests: u64) -> Self {
        Self {
            buckets: Arc::new(Mutex::new(HashMap::new())),
            max_requests,
        }
    }

    /// Returns `true` if a request from `ip` is under the rate limit, `false` otherwise.
    #[must_use]
    #[allow(clippy::significant_drop_tightening)]
    pub fn check(&self, ip: &str) -> bool {
        let now = Instant::now();
        let mut buckets = self
            .buckets
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        let (count, window_start) = buckets.entry(ip.to_string()).or_insert((0, now));

        // Reset window if more than 60 seconds have passed
        if now.duration_since(*window_start).as_secs() >= 60 {
            *count = 0;
            *window_start = now;
        }

        if *count >= self.max_requests {
            false
        } else {
            *count += 1;
            true
        }
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
