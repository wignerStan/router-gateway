//! Smart Router for LLM request routing
//!
//! This router orchestrates the complete routing pipeline:
//! - Candidate construction
//! - Constraint filtering
//! - Utility estimation
//! - Route selection (bandit or weighted)
//! - Fallback planning

use crate::bandit::BanditPolicy;
use crate::candidate::{CandidateBuilder, RouteCandidate};
use crate::classification::ClassifiedRequest;
use crate::fallback::{FallbackConfig, FallbackPlanner, FallbackRoute};
use crate::filtering::ConstraintFilter;
use crate::health::HealthManager;
use crate::metrics::MetricsCollector;
use crate::selector::SmartSelector;
use crate::session::SessionAffinityManager;
use crate::utility::UtilityEstimator;
use crate::weight::{AuthInfo, WeightCalculator};
use model_registry::ModelInfo;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Route plan with primary and fallback routes
#[derive(Debug, Clone)]
pub struct RoutePlan {
    /// Selected primary route
    pub primary: Option<RoutePlanItem>,
    /// Fallback routes in order
    pub fallbacks: Vec<FallbackRoute>,
    /// Total candidates considered
    pub total_candidates: usize,
    /// Candidates after filtering
    pub filtered_candidates: usize,
}

/// Single route plan item
#[derive(Debug, Clone)]
pub struct RoutePlanItem {
    /// Credential ID to use
    pub credential_id: String,
    /// Model ID to use
    pub model_id: String,
    /// Provider name
    pub provider: String,
    /// Estimated utility for this route
    pub utility: f64,
    /// Calculated weight for this route
    pub weight: f64,
}

/// Router configuration
#[derive(Debug, Clone)]
pub struct RouterConfig {
    /// Whether to use bandit policy for selection
    pub use_bandit: bool,
    /// Maximum number of fallback routes
    pub max_fallbacks: usize,
    /// Minimum number of fallback routes
    pub min_fallbacks: usize,
    /// Enable provider diversity in fallbacks
    pub enable_provider_diversity: bool,
    /// Enable session affinity
    pub enable_session_affinity: bool,
}

impl Default for RouterConfig {
    fn default() -> Self {
        Self {
            use_bandit: true,
            max_fallbacks: 5,
            min_fallbacks: 2,
            enable_provider_diversity: true,
            enable_session_affinity: true,
        }
    }
}

/// Smart Router for intelligent LLM request routing
///
/// Orchestrates the complete routing pipeline:
/// 1. Build route candidates from credentials and models
/// 2. Filter candidates through hard constraints
/// 3. Estimate utility for each candidate
/// 4. Select best route (bandit or weighted)
/// 5. Generate fallback routes
pub struct Router {
    /// Candidate builder
    candidate_builder: CandidateBuilder,
    /// Constraint filter
    constraint_filter: ConstraintFilter,
    /// Utility estimator
    utility_estimator: UtilityEstimator,
    /// Bandit policy for selection (wrapped for interior mutability)
    bandit_policy: Arc<Mutex<BanditPolicy>>,
    /// Smart selector for weighted selection
    selector: SmartSelector,
    /// Fallback planner
    fallback_planner: FallbackPlanner,
    /// Session affinity manager
    session_manager: SessionAffinityManager,
    /// Router configuration
    config: RouterConfig,
}

impl Router {
    /// Create a new Router
    pub fn new() -> Self {
        Self::with_config(RouterConfig::default())
    }

    /// Create a new Router with custom configuration
    pub fn with_config(config: RouterConfig) -> Self {
        Self {
            candidate_builder: CandidateBuilder::new(),
            constraint_filter: ConstraintFilter::new(),
            utility_estimator: UtilityEstimator::new(),
            bandit_policy: Arc::new(Mutex::new(BanditPolicy::new())),
            selector: SmartSelector::new(crate::config::SmartRoutingConfig::default()),
            fallback_planner: FallbackPlanner::with_config(FallbackConfig {
                max_fallbacks: config.max_fallbacks,
                min_fallbacks: config.min_fallbacks,
                enable_provider_diversity: config.enable_provider_diversity,
                prefer_diverse_providers: config.enable_provider_diversity,
            }),
            session_manager: SessionAffinityManager::new(),
            config,
        }
    }

    /// Add a credential with its associated models
    pub fn add_credential(&mut self, credential_id: String, model_ids: Vec<String>) -> &mut Self {
        self.candidate_builder
            .add_credential(credential_id, model_ids);
        self
    }

    /// Set model information
    pub fn set_model(&mut self, model_id: String, info: ModelInfo) -> &mut Self {
        self.candidate_builder.set_model(model_id, info);
        self
    }

    /// Add a disabled provider
    pub fn add_disabled_provider(&mut self, provider: String) -> &mut Self {
        self.constraint_filter.add_disabled_provider(provider);
        self
    }

    /// Set session ID for affinity
    pub fn set_session_id(&mut self, _session_id: String) {
        // Session affinity is handled during planning
        // This is a placeholder for future implementation
    }

    /// Plan a route for the given request
    ///
    /// # Arguments
    /// * `request` - Classified request with routing context
    /// * `auths` - Available auth credentials
    /// * `session_id` - Optional session ID for affinity
    ///
    /// # Returns
    /// Complete route plan with primary and fallback routes
    pub async fn plan(
        &self,
        request: &ClassifiedRequest,
        auths: Vec<AuthInfo>,
        session_id: Option<&str>,
    ) -> RoutePlan {
        // Step 1: Build candidates
        let candidates = self.candidate_builder.build_candidates(request);
        let total_candidates = candidates.len();

        // Step 2: Filter candidates through constraints
        let filtered = self.constraint_filter.filter(candidates, request);
        let filtered_count = filtered.len();

        if filtered.is_empty() {
            return RoutePlan {
                primary: None,
                fallbacks: Vec::new(),
                total_candidates,
                filtered_candidates: filtered_count,
            };
        }

        // Step 3: Calculate utility for each candidate
        let candidates_with_utility = self.calculate_utilities(&filtered).await;

        // Step 4: Check session affinity
        let selected = if self.config.enable_session_affinity {
            if let Some(sid) = session_id {
                self.select_with_affinity(&candidates_with_utility, sid)
                    .await
            } else {
                self.select_best(&candidates_with_utility).await
            }
        } else {
            self.select_best(&candidates_with_utility).await
        };

        // Step 5: Generate fallback routes
        let primary_id = selected.as_ref().map(|s| s.credential_id.clone());
        let auth_infos: Vec<_> = auths
            .into_iter()
            .filter_map(|auth| {
                // Only include auths that match our candidates
                filtered
                    .iter()
                    .find(|c| c.credential_id == auth.id)
                    .map(|_| auth)
            })
            .collect();

        let fallbacks = self
            .fallback_planner
            .generate_fallbacks(
                auth_infos,
                primary_id,
                &self.selector,
                self.selector.metrics(),
                self.selector.health(),
            )
            .await;

        RoutePlan {
            primary: selected,
            fallbacks,
            total_candidates,
            filtered_candidates: filtered_count,
        }
    }

    /// Calculate utility for all candidates in parallel
    async fn calculate_utilities(
        &self,
        candidates: &[RouteCandidate],
    ) -> Vec<(RouteCandidate, f64)> {
        use futures::future::join_all;

        let futures: Vec<_> = candidates
            .iter()
            .map(|candidate| async {
                // Get metrics for this credential
                let metrics = self.metrics().get_metrics(&candidate.credential_id).await;

                // Calculate utility
                let utility = self.utility_estimator.estimate_utility(metrics.as_ref());
                (candidate.clone(), utility)
            })
            .collect();

        join_all(futures).await
    }

    /// Select best route with session affinity
    async fn select_with_affinity(
        &self,
        candidates: &[(RouteCandidate, f64)],
        session_id: &str,
    ) -> Option<RoutePlanItem> {
        // Check if session has preferred provider
        if let Some(preferred_provider) = self
            .session_manager
            .get_preferred_provider(session_id)
            .await
        {
            // Try to find a candidate from the preferred provider
            for (candidate, utility) in candidates {
                if candidate.provider == preferred_provider {
                    // Calculate weight
                    let metrics = self.metrics().get_metrics(&candidate.credential_id).await;
                    let health = self.health().get_status(&candidate.credential_id).await;

                    let auth_info = AuthInfo {
                        id: candidate.credential_id.clone(),
                        priority: Some(0),
                        quota_exceeded: false,
                        unavailable: false,
                        model_states: Vec::new(),
                    };

                    let weight_config = self.selector.config().weight.clone();
                    let calculator = crate::weight::DefaultWeightCalculator::new(weight_config);
                    let calculated_weight =
                        calculator.calculate(&auth_info, metrics.as_ref(), health);

                    return Some(RoutePlanItem {
                        credential_id: candidate.credential_id.clone(),
                        model_id: candidate.model_id.clone(),
                        provider: candidate.provider.clone(),
                        utility: *utility,
                        weight: calculated_weight,
                    });
                }
            }
        }

        // No affinity match, fall back to normal selection
        self.select_best(candidates).await
    }

    /// Select best route using bandit or weighted selection
    async fn select_best(&self, candidates: &[(RouteCandidate, f64)]) -> Option<RoutePlanItem> {
        if candidates.is_empty() {
            return None;
        }

        if self.config.use_bandit {
            self.select_bandit(candidates).await
        } else {
            self.select_weighted(candidates).await
        }
    }

    /// Select using bandit policy
    async fn select_bandit(&self, candidates: &[(RouteCandidate, f64)]) -> Option<RoutePlanItem> {
        let credential_ids: Vec<String> = candidates
            .iter()
            .map(|(c, _)| c.credential_id.clone())
            .collect();
        let utilities: HashMap<String, f64> = candidates
            .iter()
            .map(|(c, u)| (c.credential_id.clone(), *u))
            .collect();

        let bandit = self.bandit_policy.lock().await;
        let selected_id = bandit.select_route_with_utility(&credential_ids, &utilities)?;
        drop(bandit);

        // Find the selected candidate
        for (candidate, utility) in candidates {
            if candidate.credential_id == selected_id {
                // Calculate weight
                let metrics = self.metrics().get_metrics(&candidate.credential_id).await;
                let health = self.health().get_status(&candidate.credential_id).await;

                let auth_info = AuthInfo {
                    id: candidate.credential_id.clone(),
                    priority: Some(0),
                    quota_exceeded: false,
                    unavailable: false,
                    model_states: Vec::new(),
                };

                let weight_config = self.selector.config().weight.clone();
                let calculator = crate::weight::DefaultWeightCalculator::new(weight_config);
                let calculated_weight = calculator.calculate(&auth_info, metrics.as_ref(), health);

                return Some(RoutePlanItem {
                    credential_id: candidate.credential_id.clone(),
                    model_id: candidate.model_id.clone(),
                    provider: candidate.provider.clone(),
                    utility: *utility,
                    weight: calculated_weight,
                });
            }
        }

        None
    }

    /// Select using weighted selection
    async fn select_weighted(&self, candidates: &[(RouteCandidate, f64)]) -> Option<RoutePlanItem> {
        // Select the candidate with highest utility
        let best = candidates
            .iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

        if let Some((candidate, utility)) = best {
            // Calculate weight
            let metrics = self.metrics().get_metrics(&candidate.credential_id).await;
            let health = self.health().get_status(&candidate.credential_id).await;

            let auth_info = AuthInfo {
                id: candidate.credential_id.clone(),
                priority: Some(0),
                quota_exceeded: false,
                unavailable: false,
                model_states: Vec::new(),
            };

            let weight_config = self.selector.config().weight.clone();
            let calculator = crate::weight::DefaultWeightCalculator::new(weight_config);
            let calculated_weight = calculator.calculate(&auth_info, metrics.as_ref(), health);

            Some(RoutePlanItem {
                credential_id: candidate.credential_id.clone(),
                model_id: candidate.model_id.clone(),
                provider: candidate.provider.clone(),
                utility: *utility,
                weight: calculated_weight,
            })
        } else {
            None
        }
    }

    /// Record execution result for learning
    pub async fn record_result(
        &self,
        credential_id: &str,
        success: bool,
        latency_ms: f64,
        status_code: i32,
        utility: f64,
    ) {
        // Record in metrics
        self.selector
            .metrics()
            .record_result(credential_id, success, latency_ms, status_code)
            .await;

        // Record in health manager
        self.selector
            .health()
            .update_from_result(credential_id, success, status_code)
            .await;

        // Record in bandit policy
        let mut bandit_policy = self.bandit_policy.lock().await;
        bandit_policy.record_result(credential_id, success, utility);
    }

    /// Get metrics collector
    pub fn metrics(&self) -> &MetricsCollector {
        self.selector.metrics()
    }

    /// Get health manager
    pub fn health(&self) -> &HealthManager {
        self.selector.health()
    }

    /// Get session manager
    pub fn session_manager(&self) -> &SessionAffinityManager {
        &self.session_manager
    }

    /// Get bandit policy (returns reference to the Arc for shared access)
    pub fn bandit_policy(&self) -> &Arc<Mutex<BanditPolicy>> {
        &self.bandit_policy
    }

    /// Get config
    pub fn config(&self) -> &RouterConfig {
        &self.config
    }
}

impl Default for Router {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for Router {
    fn clone(&self) -> Self {
        Self {
            candidate_builder: CandidateBuilder::new(),
            constraint_filter: ConstraintFilter::new(),
            utility_estimator: UtilityEstimator::new(),
            bandit_policy: Arc::clone(&self.bandit_policy),
            selector: SmartSelector::new(crate::config::SmartRoutingConfig::default()),
            fallback_planner: FallbackPlanner::with_config(FallbackConfig {
                max_fallbacks: self.config.max_fallbacks,
                min_fallbacks: self.config.min_fallbacks,
                enable_provider_diversity: self.config.enable_provider_diversity,
                prefer_diverse_providers: self.config.enable_provider_diversity,
            }),
            session_manager: SessionAffinityManager::new(),
            config: self.config.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classification::{QualityPreference, RequestFormat, RequiredCapabilities};
    use model_registry::{DataSource, ModelCapabilities, RateLimits};

    fn create_test_model(id: &str, provider: &str, context_window: usize) -> ModelInfo {
        ModelInfo {
            id: id.to_string(),
            name: format!("Test Model {}", id),
            provider: provider.to_string(),
            context_window,
            max_output_tokens: 4096,
            input_price_per_million: 1.0,
            output_price_per_million: 2.0,
            capabilities: ModelCapabilities {
                streaming: true,
                tools: true,
                vision: true,
                thinking: false,
            },
            rate_limits: RateLimits {
                requests_per_minute: 60,
                tokens_per_minute: 90000,
            },
            source: DataSource::Static,
        }
    }

    fn create_test_request(estimated_tokens: u32) -> ClassifiedRequest {
        ClassifiedRequest {
            required_capabilities: RequiredCapabilities::default(),
            estimated_tokens,
            format: RequestFormat::OpenAI,
            quality_preference: QualityPreference::Balanced,
        }
    }

    fn create_test_auth(id: &str) -> AuthInfo {
        AuthInfo {
            id: id.to_string(),
            priority: Some(0),
            quota_exceeded: false,
            unavailable: false,
            model_states: Vec::new(),
        }
    }

    #[tokio::test]
    async fn test_router_creation() {
        let router = Router::new();
        let _router2 = router.clone();
    }

    #[tokio::test]
    async fn test_router_default() {
        let _router = Router::default();
        let _router2 = Router::new();
        // Both should be equivalent
    }

    #[tokio::test]
    async fn test_router_add_credential() {
        let mut router = Router::new();
        router.add_credential("cred-1".to_string(), vec!["claude-3-opus".to_string()]);
        router.set_model(
            "claude-3-opus".to_string(),
            create_test_model("claude-3-opus", "anthropic", 200000),
        );
    }

    #[tokio::test]
    async fn test_router_plan_with_valid_candidates() {
        let mut router = Router::new();
        router.add_credential("cred-1".to_string(), vec!["claude-3-opus".to_string()]);
        router.set_model(
            "claude-3-opus".to_string(),
            create_test_model("claude-3-opus", "anthropic", 200000),
        );

        let request = create_test_request(1000);
        let auths = vec![create_test_auth("cred-1")];

        // Initialize metrics
        router.metrics().initialize_auth("cred-1").await;

        let plan = router.plan(&request, auths, None).await;

        assert!(plan.primary.is_some());
        assert_eq!(plan.total_candidates, 1);
        assert_eq!(plan.filtered_candidates, 1);
    }

    #[tokio::test]
    async fn test_router_plan_with_no_credentials() {
        let router = Router::new();

        let request = create_test_request(1000);
        let auths: Vec<AuthInfo> = vec![];

        let plan = router.plan(&request, auths, None).await;

        assert!(plan.primary.is_none());
        assert_eq!(plan.total_candidates, 0);
        assert_eq!(plan.filtered_candidates, 0);
    }

    #[tokio::test]
    async fn test_router_plan_with_filtered_candidates() {
        let mut router = Router::new();
        router.add_credential("cred-1".to_string(), vec!["gpt-4".to_string()]);
        // Set a model with small context window
        router.set_model(
            "gpt-4".to_string(),
            create_test_model("gpt-4", "openai", 10000),
        );

        // Request exceeds context window
        let request = create_test_request(50000);
        let auths = vec![create_test_auth("cred-1")];

        let plan = router.plan(&request, auths, None).await;

        // Should be filtered due to context overflow
        assert!(plan.primary.is_none());
        assert_eq!(plan.total_candidates, 1);
        assert_eq!(plan.filtered_candidates, 0);
    }

    #[tokio::test]
    async fn test_router_plan_generates_fallbacks() {
        let mut router = Router::new();
        router.add_credential("cred-1".to_string(), vec!["claude-3-opus".to_string()]);
        router.add_credential("cred-2".to_string(), vec!["gpt-4".to_string()]);
        router.set_model(
            "claude-3-opus".to_string(),
            create_test_model("claude-3-opus", "anthropic", 200000),
        );
        router.set_model(
            "gpt-4".to_string(),
            create_test_model("gpt-4", "openai", 128000),
        );

        let request = create_test_request(1000);
        let auths = vec![create_test_auth("cred-1"), create_test_auth("cred-2")];

        // Initialize metrics
        router.metrics().initialize_auth("cred-1").await;
        router.metrics().initialize_auth("cred-2").await;

        let plan = router.plan(&request, auths, None).await;

        assert!(plan.primary.is_some());
        assert!(!plan.fallbacks.is_empty());
    }

    #[tokio::test]
    async fn test_router_clone_independence() {
        let mut router1 = Router::new();
        router1.add_credential("cred-1".to_string(), vec!["claude-3-opus".to_string()]);
        router1.set_model(
            "claude-3-opus".to_string(),
            create_test_model("claude-3-opus", "anthropic", 200000),
        );

        let mut router2 = router1.clone();

        // Both should be valid independent instances, but clone creates fresh state
        // So we need to register credentials on router2 as well
        router2.add_credential("cred-1".to_string(), vec!["claude-3-opus".to_string()]);
        router2.set_model(
            "claude-3-opus".to_string(),
            create_test_model("claude-3-opus", "anthropic", 200000),
        );

        let request = create_test_request(1000);
        let auths = vec![create_test_auth("cred-1")];

        router2.metrics().initialize_auth("cred-1").await;
        let plan = router2.plan(&request, auths, None).await;

        assert!(plan.primary.is_some());
    }

    #[tokio::test]
    async fn test_router_record_result() {
        let router = Router::new();

        // Record a result
        router.record_result("cred-1", true, 100.0, 200, 0.8).await;

        // Verify bandit policy recorded it
        let bandit = router.bandit_policy().lock().await;
        let stats = bandit.get_stats("cred-1");
        assert!(stats.is_some());
    }

    #[tokio::test]
    async fn test_router_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Router>();
    }

    #[tokio::test]
    async fn test_router_disabled_provider() {
        let mut router = Router::new();
        router.add_disabled_provider("blocked-provider".to_string());
        router.add_credential("cred-1".to_string(), vec!["model-1".to_string()]);
        router.set_model(
            "model-1".to_string(),
            create_test_model("model-1", "blocked-provider", 200000),
        );

        let request = create_test_request(1000);
        let auths = vec![create_test_auth("cred-1")];

        let plan = router.plan(&request, auths, None).await;

        // Should be filtered due to disabled provider
        assert!(plan.primary.is_none());
    }

    #[tokio::test]
    async fn test_route_plan_item_fields() {
        let mut router = Router::new();
        router.add_credential("cred-1".to_string(), vec!["claude-3-opus".to_string()]);
        router.set_model(
            "claude-3-opus".to_string(),
            create_test_model("claude-3-opus", "anthropic", 200000),
        );

        let request = create_test_request(1000);
        let auths = vec![create_test_auth("cred-1")];

        router.metrics().initialize_auth("cred-1").await;

        let plan = router.plan(&request, auths, None).await;

        assert!(plan.primary.is_some());
        let primary = plan.primary.unwrap();
        assert_eq!(primary.credential_id, "cred-1");
        assert_eq!(primary.model_id, "claude-3-opus");
        assert_eq!(primary.provider, "anthropic");
        assert!(primary.utility >= 0.0);
        assert!(primary.weight >= 0.0);
    }

    #[tokio::test]
    async fn test_router_config_defaults() {
        let config = RouterConfig::default();
        assert!(config.use_bandit);
        assert_eq!(config.max_fallbacks, 5);
        assert_eq!(config.min_fallbacks, 2);
        assert!(config.enable_provider_diversity);
        assert!(config.enable_session_affinity);
    }

    #[tokio::test]
    async fn test_router_with_custom_config() {
        let config = RouterConfig {
            use_bandit: false,
            max_fallbacks: 3,
            min_fallbacks: 1,
            enable_provider_diversity: false,
            enable_session_affinity: false,
        };

        let router = Router::with_config(config);
        assert!(!router.config().use_bandit);
        assert_eq!(router.config().max_fallbacks, 3);
    }

    #[tokio::test]
    async fn test_router_multiple_credentials() {
        let mut router = Router::new();
        router.add_credential("cred-1".to_string(), vec!["claude-3-opus".to_string()]);
        router.add_credential("cred-2".to_string(), vec!["gpt-4".to_string()]);
        router.add_credential("cred-3".to_string(), vec!["gemini-pro".to_string()]);
        router.set_model(
            "claude-3-opus".to_string(),
            create_test_model("claude-3-opus", "anthropic", 200000),
        );
        router.set_model(
            "gpt-4".to_string(),
            create_test_model("gpt-4", "openai", 128000),
        );
        router.set_model(
            "gemini-pro".to_string(),
            create_test_model("gemini-pro", "google", 128000),
        );

        let request = create_test_request(1000);
        let auths = vec![
            create_test_auth("cred-1"),
            create_test_auth("cred-2"),
            create_test_auth("cred-3"),
        ];

        for auth in &auths {
            router.metrics().initialize_auth(&auth.id).await;
        }

        let plan = router.plan(&request, auths, None).await;

        assert!(plan.primary.is_some());
        assert_eq!(plan.total_candidates, 3);
        assert!(plan.filtered_candidates >= 1);
    }

    // ============================================================
    // Edge Case Tests for Router Orchestration
    // ============================================================

    #[tokio::test]
    async fn test_router_session_affinity_disabled_uses_normal_selection() {
        let config = RouterConfig {
            enable_session_affinity: false,
            use_bandit: false, // Use weighted for deterministic testing
            ..Default::default()
        };
        let mut router = Router::with_config(config);
        router.add_credential("cred-1".to_string(), vec!["claude-3-opus".to_string()]);
        router.add_credential("cred-2".to_string(), vec!["gpt-4".to_string()]);
        router.set_model(
            "claude-3-opus".to_string(),
            create_test_model("claude-3-opus", "anthropic", 200000),
        );
        router.set_model(
            "gpt-4".to_string(),
            create_test_model("gpt-4", "openai", 128000),
        );

        let request = create_test_request(1000);
        let auths = vec![create_test_auth("cred-1"), create_test_auth("cred-2")];

        for auth in &auths {
            router.metrics().initialize_auth(&auth.id).await;
        }

        // Plan with session_id - should be ignored since affinity disabled
        let plan = router
            .plan(&request, auths.clone(), Some("test-session"))
            .await;

        assert!(
            plan.primary.is_some(),
            "Should select a route even with session affinity disabled"
        );
    }

    #[tokio::test]
    async fn test_router_bandit_disabled_uses_weighted_selection() {
        let config = RouterConfig {
            use_bandit: false,
            ..Default::default()
        };
        let mut router = Router::with_config(config);
        router.add_credential("cred-1".to_string(), vec!["claude-3-opus".to_string()]);
        router.set_model(
            "claude-3-opus".to_string(),
            create_test_model("claude-3-opus", "anthropic", 200000),
        );

        let request = create_test_request(1000);
        let auths = vec![create_test_auth("cred-1")];

        router.metrics().initialize_auth("cred-1").await;

        let plan = router.plan(&request, auths, None).await;

        assert!(
            plan.primary.is_some(),
            "Should select route with bandit disabled"
        );
        let primary = plan.primary.unwrap();
        // With weighted selection (no bandit), utility determines selection
        assert!(primary.utility >= 0.0);
    }

    #[tokio::test]
    async fn test_router_generates_correct_number_of_fallbacks() {
        let config = RouterConfig {
            max_fallbacks: 3,
            min_fallbacks: 2,
            ..Default::default()
        };
        let mut router = Router::with_config(config);
        router.add_credential("cred-1".to_string(), vec!["claude-3-opus".to_string()]);
        router.add_credential("cred-2".to_string(), vec!["gpt-4".to_string()]);
        router.add_credential("cred-3".to_string(), vec!["gemini-pro".to_string()]);
        router.add_credential("cred-4".to_string(), vec!["llama-3".to_string()]);
        router.set_model(
            "claude-3-opus".to_string(),
            create_test_model("claude-3-opus", "anthropic", 200000),
        );
        router.set_model(
            "gpt-4".to_string(),
            create_test_model("gpt-4", "openai", 128000),
        );
        router.set_model(
            "gemini-pro".to_string(),
            create_test_model("gemini-pro", "google", 128000),
        );
        router.set_model(
            "llama-3".to_string(),
            create_test_model("llama-3", "meta", 128000),
        );

        let request = create_test_request(1000);
        let auths = vec![
            create_test_auth("cred-1"),
            create_test_auth("cred-2"),
            create_test_auth("cred-3"),
            create_test_auth("cred-4"),
        ];

        for auth in &auths {
            router.metrics().initialize_auth(&auth.id).await;
        }

        let plan = router.plan(&request, auths, None).await;

        assert!(plan.primary.is_some());
        // Fallbacks should be <= max_fallbacks (3)
        assert!(
            plan.fallbacks.len() <= 3,
            "Fallbacks should not exceed max_fallbacks"
        );
    }

    #[tokio::test]
    async fn test_router_handles_credential_model_mismatch_gracefully() {
        let mut router = Router::new();
        // Register credential with one model
        router.add_credential("cred-1".to_string(), vec!["claude-3-opus".to_string()]);
        // But don't register the model info - this simulates mismatch

        let request = create_test_request(1000);
        // Auth references non-existent model
        let auths = vec![create_test_auth("cred-1")];

        let plan = router.plan(&request, auths, None).await;

        // Should handle gracefully - either no route or empty plan
        // The candidate builder will create no candidates without model info
        assert_eq!(plan.total_candidates, 0);
    }

    #[tokio::test]
    async fn test_router_record_result_updates_all_subsystems() {
        let router = Router::new();

        // Initialize the credential in metrics first
        router.metrics().initialize_auth("cred-1").await;

        // Record a result
        router
            .record_result("cred-1", true, 150.0, 200, 0.9)
            .await;

        // Verify metrics were updated
        let metrics = router.metrics().get_metrics("cred-1").await;
        assert!(
            metrics.is_some(),
            "Metrics should be recorded"
        );
        let m = metrics.unwrap();
        assert_eq!(m.total_requests, 1);
        assert_eq!(m.success_count, 1);
        assert_eq!(m.avg_latency_ms, 150.0);

        // Verify health was updated (should be healthy)
        let health_status = router.health().get_status("cred-1").await;
        assert_eq!(
            health_status,
            crate::health::HealthStatus::Healthy,
            "Health should be healthy after success"
        );

        // Verify bandit policy was updated
        let bandit = router.bandit_policy().lock().await;
        let stats = bandit.get_stats("cred-1");
        assert!(
            stats.is_some(),
            "Bandit stats should be recorded"
        );
        let s = stats.unwrap();
        assert_eq!(s.pulls, 1);
        assert_eq!(s.last_utility, 0.9);
    }

    #[tokio::test]
    async fn test_router_plan_with_all_credentials_unavailable() {
        let mut router = Router::new();
        router.add_credential("cred-1".to_string(), vec!["claude-3-opus".to_string()]);
        router.set_model(
            "claude-3-opus".to_string(),
            create_test_model("claude-3-opus", "anthropic", 200000),
        );

        let request = create_test_request(1000);
        // Create unavailable auth
        let mut auth = create_test_auth("cred-1");
        auth.unavailable = true;
        let auths = vec![auth];

        router.metrics().initialize_auth("cred-1").await;

        let plan = router.plan(&request, auths, None).await;

        // Should still get a primary candidate (unavailable affects weight, not filtering)
        // The candidate builder doesn't filter by unavailable status
        assert!(
            plan.primary.is_some() || plan.filtered_candidates == 0,
            "Router should handle unavailable auths"
        );
    }

    #[tokio::test]
    async fn test_router_with_zero_max_fallbacks() {
        let config = RouterConfig {
            max_fallbacks: 0,
            min_fallbacks: 0,
            ..Default::default()
        };
        let mut router = Router::with_config(config);
        router.add_credential("cred-1".to_string(), vec!["claude-3-opus".to_string()]);
        router.add_credential("cred-2".to_string(), vec!["gpt-4".to_string()]);
        router.set_model(
            "claude-3-opus".to_string(),
            create_test_model("claude-3-opus", "anthropic", 200000),
        );
        router.set_model(
            "gpt-4".to_string(),
            create_test_model("gpt-4", "openai", 128000),
        );

        let request = create_test_request(1000);
        let auths = vec![create_test_auth("cred-1"), create_test_auth("cred-2")];

        for auth in &auths {
            router.metrics().initialize_auth(&auth.id).await;
        }

        let plan = router.plan(&request, auths, None).await;

        assert!(plan.primary.is_some());
        assert!(
            plan.fallbacks.is_empty(),
            "Zero max_fallbacks should return no fallbacks"
        );
    }

    #[tokio::test]
    async fn test_router_fallback_count_with_limited_candidates() {
        let config = RouterConfig {
            max_fallbacks: 10,
            min_fallbacks: 5, // More than available
            ..Default::default()
        };
        let mut router = Router::with_config(config);
        // Only 2 credentials available
        router.add_credential("cred-1".to_string(), vec!["claude-3-opus".to_string()]);
        router.add_credential("cred-2".to_string(), vec!["gpt-4".to_string()]);
        router.set_model(
            "claude-3-opus".to_string(),
            create_test_model("claude-3-opus", "anthropic", 200000),
        );
        router.set_model(
            "gpt-4".to_string(),
            create_test_model("gpt-4", "openai", 128000),
        );

        let request = create_test_request(1000);
        let auths = vec![create_test_auth("cred-1"), create_test_auth("cred-2")];

        for auth in &auths {
            router.metrics().initialize_auth(&auth.id).await;
        }

        let plan = router.plan(&request, auths, None).await;

        assert!(plan.primary.is_some());
        // Should return available fallbacks, not min_fallbacks
        assert!(
            plan.fallbacks.len() <= 2,
            "Should return available auths as fallbacks, not min_fallbacks"
        );
    }
}
