//! Model metadata registry with multi-dimension categorization and routing policy.
//!
//! Provides model discovery, capability classification, and policy-based routing
//! rules. Used by the gateway to select optimal credentials based on model
//! characteristics, provider constraints, and cost preferences.

pub mod categories;
pub mod fetcher;
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

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("model not found: {0}")]
    ModelNotFound(String),
    #[error("policy error: {0}")]
    Policy(String),
    #[error("cannot parse model ID: {0}")]
    InvalidModelId(String),
}

pub type Result<T> = std::result::Result<T, Error>;
