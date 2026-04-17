//! Smart Router for LLM request routing
//!
//! This router orchestrates the complete routing pipeline:
//! - Candidate construction
//! - Constraint filtering
//! - Utility estimation
//! - Route selection (bandit or weighted)
//! - Fallback planning

use crate::routing::bandit::BanditPolicy;
use crate::routing::candidate::CandidateBuilder;
use crate::routing::fallback::{FallbackPlanner, FallbackRoute};
use crate::routing::filtering::ConstraintFilter;
use crate::routing::selector::SmartSelector;
use crate::routing::session::SessionAffinityManager;
use crate::routing::utility::UtilityEstimator;
use std::sync::Arc;
use tokio::sync::Mutex;

mod dispatch;

#[cfg(test)]
mod tests;

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
