//! Intelligent credential selection for LLM request routing.
//!
//! Provides health-aware, latency-optimized routing based on success rates,
//! configurable weight factors, and multiple selection strategies including
//! weighted random, time-aware, quota-aware, and adaptive routing.

pub mod bandit;
pub mod candidate;
pub mod classification;
pub mod config;
pub mod executor;
pub mod fallback;
pub mod filtering;
pub mod health;
pub mod history;
pub mod metrics;
pub mod outcome;
pub mod policy_weight;
pub mod reasoning;
pub mod router;
pub mod selector;
pub mod session;
pub mod sqlite;
pub mod statistics;
pub mod utility;
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

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Sqlite(#[from] SqliteError),
    #[error("configuration error: {0}")]
    Config(String),
    #[error("no candidates available for routing")]
    NoCandidates,
}

pub type Result<T> = std::result::Result<T, Error>;
