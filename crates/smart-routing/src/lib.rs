//! Intelligent credential selection for LLM request routing.
//!
//! Provides health-aware, latency-optimized routing based on success rates,
//! configurable weight factors, and multiple selection strategies including
//! weighted random, time-aware, quota-aware, and adaptive routing.

/// Multi-armed bandit exploration strategies.
pub mod bandit;
/// Route candidate evaluation and scoring.
pub mod candidate;
/// Request classification for routing decisions.
pub mod classification;
/// Routing configuration types.
pub mod config;
/// Route execution and result handling.
pub mod executor;
/// Fallback planning and route recovery.
pub mod fallback;
/// Constraint filtering for candidate selection.
pub mod filtering;
/// Credential health tracking and status management.
pub mod health;
/// Attempt history and decision tracking.
pub mod history;
/// Credential metrics collection and aggregation.
pub mod metrics;
/// Execution outcome recording.
pub mod outcome;
/// Policy-aware weight calculation.
pub mod policy_weight;
/// Reasoning capability inference.
pub mod reasoning;
/// Request routing and dispatch.
pub mod router;
/// Smart credential selector.
pub mod selector;
/// Session affinity management.
pub mod session;
/// SQLite-backed persistence for metrics and health.
pub mod sqlite;
/// Route statistics aggregation and time-bucket analysis.
pub mod statistics;
/// Utility estimation for route selection.
pub mod utility;
/// Credential weight calculation strategies.
pub mod weight;

pub use bandit::{BanditConfig, BanditPolicy, RouteStats};
pub use candidate::{
    check_capability_support, CandidateBuilder, CapabilitySupport, RouteCandidate, TokenFitStatus,
};
pub use classification::{
    ClassifiedRequest, FormatDetector, QualityPreference, RequestClassifier, RequestFormat,
    RequiredCapabilities, TokenEstimator,
};
pub use config::{
    HealthConfig, PolicyConfig, QuotaAwareConfig, SmartRoutingConfig, TimeAwareConfig, WeightConfig,
};
pub use executor::{ExecutionResult, ExecutorConfig, RouteExecutor};
pub use fallback::{FallbackConfig, FallbackPlanner, FallbackRoute};
pub use filtering::{ConstraintFilter, FilterResult};
pub use health::{AuthHealth, HealthManager, HealthStatus};
pub use history::{
    AttemptHistory, AttemptMetrics, DecisionContext, RouteAttempt, SelectionMode, TrackingSystem,
};
pub use metrics::{AuthMetrics, MetricsCollector};
pub use outcome::{ErrorClass, ExecutionOutcome, OutcomeRecorder};
pub use policy_weight::{
    PolicyAwareWeightCalculator, PolicyWeightCalculator, WeightCalculatorFactory,
};
pub use reasoning::{ReasoningCapability, ReasoningInference, ReasoningRequest};
pub use router::Router;
pub use selector::SmartSelector;
pub use session::{SessionAffinity, SessionAffinityManager, SessionStats};
pub use sqlite::{
    SQLiteConfig, SQLiteHealthManager, SQLiteMetricsCollector, SQLiteSelector, SQLiteStore,
    SelectorStats,
};
pub use statistics::{
    BucketStatistics, ColdStartPriors, RouteStatistics, StatisticsAggregator, TimeBucket,
};
pub use utility::{UtilityConfig, UtilityEstimator};
pub use weight::{AuthInfo, DefaultWeightCalculator, ModelState, WeightCalculator};

pub use sqlite::error::SqliteError;

/// Top-level error type for the smart-routing crate.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Error from the `SQLite` backend via [`SqliteError`].
    #[error(transparent)]
    Sqlite(#[from] SqliteError),
    /// Configuration error.
    #[error("configuration error: {0}")]
    Config(String),
    /// No candidates available for routing.
    #[error("no candidates available for routing")]
    NoCandidates,
}

/// Result type for the smart-routing crate.
pub type Result<T> = std::result::Result<T, Error>;
