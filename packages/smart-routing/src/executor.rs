//! Route executor with retry and fallback
//!
//! This module executes route plans with automatic retry on failure,
//! fallback triggering, retry budget management, and loop guard.

use crate::fallback::FallbackRoute;
use crate::health::HealthManager;
use crate::metrics::MetricsCollector;
use crate::router::RoutePlanItem;
use std::collections::HashSet;

/// Execution result
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    /// Whether execution succeeded
    pub success: bool,
    /// Credential ID that succeeded
    pub credential_id: Option<String>,
    /// Model ID used
    pub model_id: Option<String>,
    /// Number of attempts made
    pub attempts: u32,
    /// Total latency in milliseconds
    pub total_latency_ms: f64,
    /// Final status code
    pub status_code: Option<i32>,
    /// Error message if failed
    pub error: Option<String>,
}

/// Executor configuration
#[derive(Debug, Clone)]
pub struct ExecutorConfig {
    /// Maximum number of retry attempts (excluding primary)
    pub max_retries: u32,
    /// Timeout for each attempt in milliseconds
    pub timeout_ms: u64,
    /// Enable provider diversity in retries
    pub enable_provider_diversity: bool,
    /// Retryable status codes (default: 429, 500-599)
    pub retryable_status_codes: HashSet<i32>,
}

impl Default for ExecutorConfig {
    fn default() -> Self {
        let mut retryable_status_codes = HashSet::new();
        // Rate limit (429)
        retryable_status_codes.insert(429);
        // Server errors (500-599)
        for code in 500..600 {
            retryable_status_codes.insert(code);
        }

        Self {
            max_retries: 3,
            timeout_ms: 30000,
            enable_provider_diversity: true,
            retryable_status_codes,
        }
    }
}

/// Route executor with retry and fallback
pub struct RouteExecutor {
    config: ExecutorConfig,
    metrics: MetricsCollector,
    health: HealthManager,
}

impl RouteExecutor {
    /// Create a new route executor
    pub fn new(config: ExecutorConfig, metrics: MetricsCollector, health: HealthManager) -> Self {
        Self {
            config,
            metrics,
            health,
        }
    }

    /// Execute a route plan with retry and fallback
    ///
    /// # Scenarios (gateway-a15)
    /// - Successful primary returns response immediately
    /// - Timeout triggers fallback
    /// - Retryable errors trigger fallback
    ///
    /// # Arguments
    /// * `plan` - Route plan with primary and fallbacks
    /// * `execute_fn` - Function to execute a single route attempt
    ///
    /// # Returns
    /// Execution result with success/failure and metadata
    pub async fn execute<F, Fut>(
        &self,
        primary: Option<RoutePlanItem>,
        fallbacks: Vec<FallbackRoute>,
        mut execute_fn: F,
    ) -> ExecutionResult
    where
        F: FnMut(&RoutePlanItem) -> Fut,
        Fut: std::future::Future<Output = Result<(i32, f64), String>>,
    {
        let mut attempts = 0u32;
        let mut total_latency = 0.0;
        let mut used_providers: HashSet<String> = HashSet::new();
        let mut used_credentials: HashSet<String> = HashSet::new();

        // Try primary first
        if let Some(primary_route) = &primary {
            used_providers.insert(primary_route.provider.clone());
            used_credentials.insert(primary_route.credential_id.clone());

            match self.attempt_route(primary_route, &mut execute_fn).await {
                AttemptResult::Success {
                    status_code,
                    latency,
                } => {
                    // Record success
                    self.metrics
                        .record_result(&primary_route.credential_id, true, latency, status_code)
                        .await;
                    self.health
                        .update_from_result(&primary_route.credential_id, true, status_code)
                        .await;

                    return ExecutionResult {
                        success: true,
                        credential_id: Some(primary_route.credential_id.clone()),
                        model_id: Some(primary_route.model_id.clone()),
                        attempts: attempts + 1,
                        total_latency_ms: latency,
                        status_code: Some(status_code),
                        error: None,
                    };
                },
                AttemptResult::RetryableError {
                    status_code,
                    latency,
                    error,
                } => {
                    attempts += 1;
                    total_latency += latency;

                    // Record failure
                    self.metrics
                        .record_result(&primary_route.credential_id, false, latency, status_code)
                        .await;
                    self.health
                        .update_from_result(&primary_route.credential_id, false, status_code)
                        .await;

                    // Check if should trigger fallback (gateway-nuk)
                    if self.is_retryable(status_code) {
                        // Continue to fallbacks
                    } else {
                        // Non-retryable error (e.g., auth error) - return immediately
                        return ExecutionResult {
                            success: false,
                            credential_id: Some(primary_route.credential_id.clone()),
                            model_id: Some(primary_route.model_id.clone()),
                            attempts, // Already incremented above
                            total_latency_ms: total_latency,
                            status_code: Some(status_code),
                            error: Some(error),
                        };
                    }
                },
            }
        }

        // Try fallbacks with retry budget (gateway-edv)
        for fallback in &fallbacks {
            // Check retry budget
            if attempts >= self.config.max_retries + 1 {
                // Budget exhausted
                break;
            }

            // Loop guard: check if we've used this credential (gateway-u0k)
            if used_credentials.contains(&fallback.auth_id) {
                continue;
            }

            // Loop guard: enforce provider diversity (gateway-u0k)
            if self.config.enable_provider_diversity {
                if let Some(ref provider) = fallback.provider {
                    if used_providers.contains(provider) {
                        // Skip to enforce provider diversity
                        continue;
                    }
                }
            }

            // Build RoutePlanItem from FallbackRoute
            let route_item = RoutePlanItem {
                credential_id: fallback.auth_id.clone(),
                model_id: String::new(), // Will be determined by the actual execution
                provider: fallback.provider.clone().unwrap_or_default(),
                utility: fallback.weight,
                weight: fallback.weight,
            };

            // Mark as used
            used_providers.insert(route_item.provider.clone());
            used_credentials.insert(route_item.credential_id.clone());

            match self.attempt_route(&route_item, &mut execute_fn).await {
                AttemptResult::Success {
                    status_code,
                    latency,
                } => {
                    // Record success
                    self.metrics
                        .record_result(&route_item.credential_id, true, latency, status_code)
                        .await;
                    self.health
                        .update_from_result(&route_item.credential_id, true, status_code)
                        .await;

                    total_latency += latency;

                    return ExecutionResult {
                        success: true,
                        credential_id: Some(route_item.credential_id.clone()),
                        model_id: Some(route_item.model_id.clone()),
                        attempts: attempts + 1,
                        total_latency_ms: total_latency,
                        status_code: Some(status_code),
                        error: None,
                    };
                },
                AttemptResult::RetryableError {
                    status_code,
                    latency,
                    error,
                } => {
                    attempts += 1;
                    total_latency += latency;

                    // Record failure
                    self.metrics
                        .record_result(&route_item.credential_id, false, latency, status_code)
                        .await;
                    self.health
                        .update_from_result(&route_item.credential_id, false, status_code)
                        .await;

                    // Check if should continue to next fallback
                    if !self.is_retryable(status_code) {
                        // Non-retryable error - stop trying
                        return ExecutionResult {
                            success: false,
                            credential_id: Some(route_item.credential_id.clone()),
                            model_id: Some(route_item.model_id.clone()),
                            attempts, // Already incremented above
                            total_latency_ms: total_latency,
                            status_code: Some(status_code),
                            error: Some(error),
                        };
                    }
                    // Continue to next fallback
                },
            }
        }

        // All attempts exhausted
        ExecutionResult {
            success: false,
            credential_id: None,
            model_id: None,
            attempts,
            total_latency_ms: total_latency,
            status_code: None,
            error: Some("Retry budget exhausted".to_string()),
        }
    }

    /// Attempt to execute a single route
    async fn attempt_route<F, Fut>(
        &self,
        route: &RoutePlanItem,
        execute_fn: &mut F,
    ) -> AttemptResult
    where
        F: FnMut(&RoutePlanItem) -> Fut,
        Fut: std::future::Future<Output = Result<(i32, f64), String>>,
    {
        // Execute the route
        match execute_fn(route).await {
            Ok((status_code, latency)) => {
                if (200..300).contains(&status_code) {
                    AttemptResult::Success {
                        status_code,
                        latency,
                    }
                } else {
                    AttemptResult::RetryableError {
                        status_code,
                        latency,
                        error: format!("HTTP {}", status_code),
                    }
                }
            },
            Err(error) => AttemptResult::RetryableError {
                status_code: 0,
                latency: 0.0,
                error,
            },
        }
    }

    /// Check if a status code is retryable (gateway-nuk)
    ///
    /// # Scenarios
    /// - Rate limit (429) triggers fallback
    /// - Server error (5xx) triggers fallback
    /// - Auth error (401-403) does NOT trigger fallback
    fn is_retryable(&self, status_code: i32) -> bool {
        // Success codes are not retryable (we already succeeded)
        if (200..300).contains(&status_code) {
            return false;
        }

        // Auth errors (401-403) are NOT retryable
        if (401..404).contains(&status_code) {
            return false;
        }

        // Check against configured retryable codes
        self.config.retryable_status_codes.contains(&status_code)
    }

    /// Get config
    pub fn config(&self) -> &ExecutorConfig {
        &self.config
    }

    /// Get metrics collector
    pub fn metrics(&self) -> &MetricsCollector {
        &self.metrics
    }

    /// Get health manager
    pub fn health(&self) -> &HealthManager {
        &self.health
    }
}

/// Internal attempt result
enum AttemptResult {
    Success {
        status_code: i32,
        latency: f64,
    },
    RetryableError {
        status_code: i32,
        latency: f64,
        error: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::HealthConfig;

    fn create_test_route(credential_id: &str, provider: &str) -> RoutePlanItem {
        RoutePlanItem {
            credential_id: credential_id.to_string(),
            model_id: "test-model".to_string(),
            provider: provider.to_string(),
            utility: 1.0,
            weight: 1.0,
        }
    }

    fn create_test_fallback(auth_id: &str, provider: &str, position: usize) -> FallbackRoute {
        FallbackRoute {
            auth_id: auth_id.to_string(),
            position,
            weight: 1.0,
            provider: Some(provider.to_string()),
        }
    }

    #[tokio::test]
    async fn test_execute_successful_primary() {
        // Scenario: Successful primary returns response
        let config = ExecutorConfig::default();
        let metrics = MetricsCollector::new();
        let health = HealthManager::new(HealthConfig::default());

        let executor = RouteExecutor::new(config, metrics, health);

        let primary = Some(create_test_route("cred-1", "anthropic"));
        let fallbacks = vec![];

        let result = executor
            .execute(primary, fallbacks, |_route| async { Ok((200, 100.0)) })
            .await;

        assert!(result.success);
        assert_eq!(result.credential_id, Some("cred-1".to_string()));
        assert_eq!(result.attempts, 1);
    }

    #[tokio::test]
    async fn test_execute_rate_limit_triggers_fallback() {
        // Scenario: Rate limit (429) triggers fallback
        let config = ExecutorConfig::default();
        let metrics = MetricsCollector::new();
        let health = HealthManager::new(HealthConfig::default());

        let executor = RouteExecutor::new(config, metrics, health);

        let primary = Some(create_test_route("cred-1", "anthropic"));
        let fallbacks = vec![create_test_fallback("cred-2", "openai", 0)];

        let result = executor
            .execute(primary, fallbacks, |route| {
                let cred_id = route.credential_id.clone();
                async move {
                    if cred_id == "cred-1" {
                        Ok((429, 50.0)) // Rate limit on primary
                    } else {
                        Ok((200, 100.0)) // Success on fallback
                    }
                }
            })
            .await;

        assert!(result.success);
        assert_eq!(result.credential_id, Some("cred-2".to_string()));
        assert_eq!(result.attempts, 2);
    }

    #[tokio::test]
    async fn test_execute_server_error_triggers_fallback() {
        // Scenario: Server error (500) triggers fallback
        let config = ExecutorConfig::default();
        let metrics = MetricsCollector::new();
        let health = HealthManager::new(HealthConfig::default());

        let executor = RouteExecutor::new(config, metrics, health);

        let primary = Some(create_test_route("cred-1", "anthropic"));
        let fallbacks = vec![create_test_fallback("cred-2", "openai", 0)];

        let result = executor
            .execute(primary, fallbacks, |route| {
                let cred_id = route.credential_id.clone();
                async move {
                    if cred_id == "cred-1" {
                        Ok((500, 50.0)) // Server error on primary
                    } else {
                        Ok((200, 100.0)) // Success on fallback
                    }
                }
            })
            .await;

        assert!(result.success);
        assert_eq!(result.credential_id, Some("cred-2".to_string()));
        assert_eq!(result.attempts, 2);
    }

    #[tokio::test]
    async fn test_execute_auth_error_no_fallback() {
        // Scenario: Auth error (401) does NOT trigger fallback
        let config = ExecutorConfig::default();
        let metrics = MetricsCollector::new();
        let health = HealthManager::new(HealthConfig::default());

        let executor = RouteExecutor::new(config, metrics, health);

        let primary = Some(create_test_route("cred-1", "anthropic"));
        let fallbacks = vec![create_test_fallback("cred-2", "openai", 0)];

        let result = executor
            .execute(primary, fallbacks, |_route| async {
                // Always return auth error
                Ok((401, 50.0))
            })
            .await;

        assert!(!result.success);
        assert_eq!(result.credential_id, Some("cred-1".to_string()));
        assert_eq!(result.attempts, 1); // Only primary attempted
    }

    #[tokio::test]
    async fn test_retry_budget_exhausted() {
        // Scenario: Budget exhausted returns failure
        let config = ExecutorConfig {
            max_retries: 1, // Only 1 retry allowed
            ..Default::default()
        };
        let metrics = MetricsCollector::new();
        let health = HealthManager::new(HealthConfig::default());

        let executor = RouteExecutor::new(config, metrics, health);

        let primary = Some(create_test_route("cred-1", "anthropic"));
        let fallbacks = vec![
            create_test_fallback("cred-2", "openai", 0),
            create_test_fallback("cred-3", "google", 1),
        ];

        let result = executor
            .execute(primary, fallbacks, |_route| async {
                Ok((429, 50.0)) // Always rate limit
            })
            .await;

        assert!(!result.success);
        assert_eq!(result.attempts, 2); // Primary + 1 retry = budget exhausted
    }

    #[tokio::test]
    async fn test_loop_guard_same_route_blocked() {
        // Scenario: Same route blocked
        let config = ExecutorConfig::default();
        let metrics = MetricsCollector::new();
        let health = HealthManager::new(HealthConfig::default());

        let executor = RouteExecutor::new(config, metrics, health);

        let primary = Some(create_test_route("cred-1", "anthropic"));
        // Fallback with same credential as primary
        let fallbacks = vec![create_test_fallback("cred-1", "anthropic", 0)];

        let call_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));

        let result = executor
            .execute(primary, fallbacks, {
                let cc = call_count.clone();
                move |_route| {
                    let cc2 = cc.clone();
                    async move {
                        let count = cc2.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        if count == 0 {
                            Ok((429, 50.0)) // First call rate limits
                        } else {
                            Ok((200, 100.0)) // Second call would succeed
                        }
                    }
                }
            })
            .await;

        // Should fail because fallback is blocked (same credential)
        assert!(!result.success);
        assert_eq!(result.attempts, 1); // Only primary attempted
        assert_eq!(call_count.load(std::sync::atomic::Ordering::Relaxed), 1); // Fallback blocked
    }

    #[tokio::test]
    async fn test_loop_guard_provider_diversity() {
        // Scenario: Same provider triggers diversity enforcement
        let config = ExecutorConfig {
            enable_provider_diversity: true,
            ..Default::default()
        };
        let metrics = MetricsCollector::new();
        let health = HealthManager::new(HealthConfig::default());

        let executor = RouteExecutor::new(config, metrics, health);

        let primary = Some(create_test_route("cred-1", "anthropic"));
        let fallbacks = vec![
            create_test_fallback("cred-2", "anthropic", 0), // Same provider as primary
            create_test_fallback("cred-3", "openai", 1),    // Different provider
        ];

        let result = executor
            .execute(primary, fallbacks, |route| {
                let cred_id = route.credential_id.clone();
                async move {
                    if cred_id == "cred-1" {
                        Ok((429, 50.0)) // Primary rate limits
                    } else if cred_id == "cred-2" {
                        panic!("Should not try cred-2 (same provider as primary)");
                    } else {
                        Ok((200, 100.0)) // cred-3 succeeds
                    }
                }
            })
            .await;

        assert!(result.success);
        assert_eq!(result.credential_id, Some("cred-3".to_string()));
        assert_eq!(result.attempts, 2); // Primary + cred-3 (cred-2 skipped)
    }

    #[tokio::test]
    async fn test_success_within_budget_stops_retrying() {
        // Scenario: Success within budget stops retrying
        let config = ExecutorConfig {
            max_retries: 5, // Large budget
            ..Default::default()
        };
        let metrics = MetricsCollector::new();
        let health = HealthManager::new(HealthConfig::default());

        let executor = RouteExecutor::new(config, metrics, health);

        let primary = Some(create_test_route("cred-1", "anthropic"));
        let fallbacks = vec![
            create_test_fallback("cred-2", "openai", 0),
            create_test_fallback("cred-3", "google", 1),
            create_test_fallback("cred-4", "cohere", 2),
        ];

        let result = executor
            .execute(primary, fallbacks, |route| {
                let cred_id = route.credential_id.clone();
                async move {
                    if cred_id == "cred-1" {
                        Ok((429, 50.0))
                    } else if cred_id == "cred-2" {
                        Ok((200, 100.0)) // Success on first fallback
                    } else {
                        panic!("Should not reach here");
                    }
                }
            })
            .await;

        assert!(result.success);
        assert_eq!(result.credential_id, Some("cred-2".to_string()));
        assert_eq!(result.attempts, 2); // Stopped after success
    }

    #[tokio::test]
    async fn test_no_primary_returns_failure() {
        // Scenario: No primary route provided
        let config = ExecutorConfig::default();
        let metrics = MetricsCollector::new();
        let health = HealthManager::new(HealthConfig::default());

        let executor = RouteExecutor::new(config, metrics, health);

        let primary: Option<RoutePlanItem> = None;
        let fallbacks = vec![create_test_fallback("cred-1", "anthropic", 0)];

        // Fallback succeeds (returns 200)
        let result = executor
            .execute(primary, fallbacks, |_route| async { Ok((200, 100.0)) })
            .await;

        // When there's no primary, fallbacks are tried
        // If fallback succeeds, result is successful
        assert!(result.success);
        assert_eq!(result.attempts, 1); // One fallback attempt
    }

    #[tokio::test]
    async fn test_is_retryable() {
        let config = ExecutorConfig::default();
        let metrics = MetricsCollector::new();
        let health = HealthManager::new(HealthConfig::default());

        let executor = RouteExecutor::new(config, metrics, health);

        // Success codes are not retryable
        assert!(!executor.is_retryable(200));
        assert!(!executor.is_retryable(204));

        // Auth errors are not retryable
        assert!(!executor.is_retryable(401));
        assert!(!executor.is_retryable(402));
        assert!(!executor.is_retryable(403));

        // Rate limit is retryable
        assert!(executor.is_retryable(429));

        // Server errors are retryable
        assert!(executor.is_retryable(500));
        assert!(executor.is_retryable(502));
        assert!(executor.is_retryable(503));
        assert!(executor.is_retryable(599));
    }

    #[tokio::test]
    async fn test_execution_result_fields() {
        let config = ExecutorConfig::default();
        let metrics = MetricsCollector::new();
        let health = HealthManager::new(HealthConfig::default());

        let executor = RouteExecutor::new(config, metrics, health);

        let primary = Some(create_test_route("cred-1", "anthropic"));
        let fallbacks = vec![];

        let result = executor
            .execute(primary, fallbacks, |_route| async { Ok((200, 150.0)) })
            .await;

        assert!(result.success);
        assert_eq!(result.credential_id, Some("cred-1".to_string()));
        assert_eq!(result.model_id, Some("test-model".to_string()));
        assert_eq!(result.attempts, 1);
        assert_eq!(result.total_latency_ms, 150.0);
        assert_eq!(result.status_code, Some(200));
        assert!(result.error.is_none());
    }
}
