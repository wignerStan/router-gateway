use crate::bandit::BanditPolicy;
use crate::candidate::RouteCandidate;
use crate::classification::ClassifiedRequest;
use crate::fallback::FallbackConfig;
use crate::health::HealthManager;
use crate::metrics::MetricsCollector;
use crate::session::SessionAffinityManager;
use crate::utility::UtilityEstimator;
use crate::weight::{AuthInfo, WeightCalculator};
use model_registry::ModelInfo;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use super::{RoutePlan, RoutePlanItem, Router, RouterConfig};

impl Router {
    /// Create a new Router
    pub fn new() -> Self {
        Self::with_config(RouterConfig::default())
    }

    /// Create a new Router with custom configuration
    pub fn with_config(config: RouterConfig) -> Self {
        Self {
            candidate_builder: crate::candidate::CandidateBuilder::new(),
            constraint_filter: crate::filtering::ConstraintFilter::new(),
            utility_estimator: UtilityEstimator::new(),
            bandit_policy: Arc::new(Mutex::new(BanditPolicy::new())),
            selector: crate::selector::SmartSelector::new(
                crate::config::SmartRoutingConfig::default(),
            ),
            fallback_planner: crate::fallback::FallbackPlanner::with_config(FallbackConfig {
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

        // Record session affinity after selection
        if self.config.enable_session_affinity {
            if let (Some(sid), Some(ref route)) = (session_id, &selected) {
                let _ = self
                    .session_manager
                    .set_provider(sid.to_string(), route.provider.clone())
                    .await;
            }
        }

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
    async fn calculate_utilities<'a>(
        &self,
        candidates: &'a [RouteCandidate],
    ) -> Vec<(&'a RouteCandidate, f64)> {
        use futures::future::join_all;

        let futures: Vec<_> = candidates
            .iter()
            .map(|candidate| async move {
                let metrics = self.metrics().get_metrics(&candidate.credential_id).await;
                let utility = self.utility_estimator.estimate_utility(metrics.as_ref());
                (candidate, utility)
            })
            .collect();

        join_all(futures).await
    }

    /// Select best route with session affinity
    async fn select_with_affinity(
        &self,
        candidates: &[(&RouteCandidate, f64)],
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
    async fn select_best(&self, candidates: &[(&RouteCandidate, f64)]) -> Option<RoutePlanItem> {
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
    async fn select_bandit(&self, candidates: &[(&RouteCandidate, f64)]) -> Option<RoutePlanItem> {
        let credential_ids: Vec<&str> = candidates
            .iter()
            .map(|(c, _)| c.credential_id.as_str())
            .collect();
        let utilities: HashMap<&str, f64> = candidates
            .iter()
            .map(|(c, u)| (c.credential_id.as_str(), *u))
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
    async fn select_weighted(
        &self,
        candidates: &[(&RouteCandidate, f64)],
    ) -> Option<RoutePlanItem> {
        // Select the candidate with highest utility
        let best = candidates
            .iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

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
            candidate_builder: self.candidate_builder.clone(),
            constraint_filter: self.constraint_filter.clone(),
            utility_estimator: self.utility_estimator.clone(),
            bandit_policy: Arc::clone(&self.bandit_policy),
            selector: self.selector.clone(),
            fallback_planner: self.fallback_planner.clone(),
            session_manager: self.session_manager.clone(),
            config: self.config.clone(),
        }
    }
}
