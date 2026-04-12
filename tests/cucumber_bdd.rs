#![allow(
    clippy::unreadable_literal,
    missing_docs,
    clippy::expect_used,
    clippy::trivial_regex,
    clippy::unused_async,
    clippy::needless_pass_by_ref_mut,
    clippy::unwrap_used,
    clippy::panic,
    clippy::used_underscore_binding,
    clippy::float_cmp
)]
// Cucumber BDD test harness for gateway
//
// Step definitions mapping to the .feature files in docs/features/:
//   - docs/features/request-classification/request-classification.feature (17 scenarios)
//   - docs/features/health-management/health-management.feature       (9 scenarios)
//   - docs/features/route-planning/route-planning.feature             (20 scenarios)
//   - docs/features/route-execution/route-execution.feature           (11 scenarios)
//   - docs/features/learning-statistics/learning-statistics.feature   (10 scenarios)
//
// Uses #[derive(World)] — NOT #[derive(WorldInit)] (removed in cucumber 0.14.0).
// harness = false is configured in Cargo.toml [[test]] entry.
//
// Run with: cargo test --test cucumber_bdd

use cucumber::World;
use gateway::routing::bandit::BanditPolicy;
use gateway::routing::classification::RequestFormat;
use gateway::routing::config::HealthConfig;
use gateway::routing::health::HealthManager;
use gateway::routing::metrics::MetricsCollector;
use gateway::routing::reasoning::ReasoningRequest;
use gateway::routing::session::SessionAffinityManager;
use gateway::routing::statistics::{ColdStartPriors, StatisticsAggregator};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

mod bdd;

// ---------------------------------------------------------------------------
// World
// ---------------------------------------------------------------------------

/// Result of classifying a request via the detectors.
#[derive(Clone, Debug)]
#[allow(dead_code)]
struct ClassificationResult {
    vision_required: bool,
    tools_required: bool,
    streaming_required: bool,
    thinking_required: bool,
    format: RequestFormat,
    estimated_tokens: u32,
    estimated_input_tokens: u32,
    estimated_output_tokens: u32,
}

/// Shared world state for BDD scenarios.
///
/// Each scenario gets a fresh instance via `Default`.
#[derive(World)]
pub struct BddWorld {
    // -- classification --
    current_request: Option<serde_json::Value>,
    reasoning_request: Option<ReasoningRequest>,
    classification_result: Option<ClassificationResult>,
    expected_output_tokens: u32,

    // -- health --
    health_manager: Option<Arc<HealthManager>>,
    health_config: HealthConfig,

    // -- metrics --
    metrics: MetricsCollector,

    // -- statistics --
    aggregator: StatisticsAggregator,
    priors: ColdStartPriors,

    // -- bandit / learning --
    bandit_policy: BanditPolicy,
    bandit_routes: Vec<String>,

    // -- session --
    session_manager: SessionAffinityManager,

    // -- execution state --
    current_auth_id: String,
    attempted_routes: HashSet<String>,
    provider_failures: HashMap<String, u32>,
    attempt_count: u32,
    retry_budget: u32,
    last_outcome_success: bool,
}

impl Default for BddWorld {
    fn default() -> Self {
        Self {
            current_request: None,
            reasoning_request: None,
            classification_result: None,
            expected_output_tokens: 0,
            health_manager: None,
            health_config: HealthConfig::default(),
            metrics: MetricsCollector::new(),
            aggregator: StatisticsAggregator::new(),
            priors: ColdStartPriors::new(),
            bandit_policy: BanditPolicy::new(),
            bandit_routes: Vec::new(),
            session_manager: SessionAffinityManager::new(),
            current_auth_id: "test-auth".to_string(),
            attempted_routes: HashSet::new(),
            provider_failures: HashMap::new(),
            attempt_count: 0,
            retry_budget: 3,
            last_outcome_success: false,
        }
    }
}

impl std::fmt::Debug for BddWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BddWorld")
            .field("current_request", &self.current_request)
            .field("classification_result", &self.classification_result)
            .field("current_auth_id", &self.current_auth_id)
            .field("attempt_count", &self.attempt_count)
            .field("retry_budget", &self.retry_budget)
            .field("bandit_routes", &self.bandit_routes)
            .field("last_outcome_success", &self.last_outcome_success)
            .finish_non_exhaustive()
    }
}

// ============================================================================
// MAIN — Wire feature files to step definitions
// ============================================================================

#[tokio::main]
async fn main() {
    BddWorld::run("docs/features").await;
}
