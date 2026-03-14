//! Intelligent credential selection for LLM request routing.
//!
//! Provides health-aware, latency-optimized routing based on success rates,
//! configurable weight factors, and multiple selection strategies including
//! weighted random, time-aware, quota-aware, and adaptive routing.

/// Multi-armed bandit exploration strategies for route optimization.
pub mod bandit;
/// Route candidate evaluation and capability checking.
pub mod candidate;
/// Request classification for routing-relevant information extraction.
pub mod classification;
/// Configuration types for smart routing behavior.
pub mod config;
/// Route execution with outcome recording.
pub mod executor;
/// Fallback planning when primary routes fail.
pub mod fallback;
/// Hard constraint filtering for route candidates.
pub mod filtering;
/// Health tracking and status management for credentials.
pub mod health;
/// Route attempt history and decision tracking.
pub mod history;
/// Credential-level metrics collection and aggregation.
pub mod metrics;
/// Execution outcome classification and recording.
pub mod outcome;
/// Policy-aware weight calculation for routing.
pub mod policy_weight;
/// Reasoning capability detection and inference.
pub mod reasoning;
/// Core routing engine that orchestrates selection.
pub mod router;
/// Credential selection strategies and smart selector.
pub mod selector;
/// Session affinity management for sticky routing.
pub mod session;
/// `SQLite` persistence for metrics, health, and weights.
pub mod sqlite;
/// Route statistics aggregation and time-bucket analysis.
pub mod statistics;
/// Utility estimation for credential value scoring.
pub mod utility;
/// Weight calculation for credential selection.
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

/// Errors produced by the smart routing engine.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A `SQLite` persistence error occurred.
    #[error(transparent)]
    Sqlite(#[from] SqliteError),
    /// A configuration error occurred.
    #[error("configuration error: {0}")]
    Config(String),
    /// No route candidates are available.
    #[error("no candidates available for routing")]
    NoCandidates,
}

/// Result type for smart routing operations.
pub type Result<T> = std::result::Result<T, Error>;
