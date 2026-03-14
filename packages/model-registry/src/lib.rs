//! Model metadata registry with multi-dimension categorization and routing policy.
//!
//! Provides model discovery, capability classification, and policy-based routing
//! rules. Used by the gateway to select optimal credentials based on model
//! characteristics, provider constraints, and cost preferences.

/// Model categorization by capability, tier, cost, context, and provider.
pub mod categories;
/// Model metadata fetching from external sources.
pub mod fetcher;
/// Core model metadata types for routing decisions.
pub mod info;
pub mod policy;
pub mod registry;

pub use categories::{
    CapabilityCategory, ContextWindowCategory, CostCategory, ModelCategorization, ProviderCategory,
    TierCategory,
};
pub use fetcher::{ModelFetcher, StaticFetcher};
pub use info::{DataSource, ModelCapabilities, ModelInfo, RateLimits};
pub use policy::templates;
pub use policy::{
    ModalityCategory, PolicyAction, PolicyCondition, PolicyConditionType, PolicyContext,
    PolicyFilters, PolicyLoadError, PolicyMatch, PolicyMatcher, PolicyRegistry, RoutingPolicy,
};
pub use registry::Registry;

/// Errors produced by the model registry.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The model ID was not found in the registry.
    #[error("model not found: {0}")]
    ModelNotFound(String),
    /// A routing policy error occurred.
    #[error("policy error: {0}")]
    Policy(String),
    /// The provided model ID could not be parsed.
    #[error("cannot parse model ID: {0}")]
    InvalidModelId(String),
}

/// Result type for registry operations.
pub type Result<T> = std::result::Result<T, Error>;
