//! Model metadata registry with multi-dimension categorization and routing policy.
//!
//! Provides model discovery, capability classification, and policy-based routing
//! rules. Used by the gateway to select optimal credentials based on model
//! characteristics, provider constraints, and cost preferences.

/// Multi-dimensional classification for model routing.
pub mod categories;
/// Model data fetching interface and static implementation.
pub mod fetcher;
/// Model metadata types, capabilities, and validation.
pub mod info;
/// Multi-dimensional routing policy configuration.
pub mod policy;
/// Thread-safe model registry with caching and coalesced fetches.
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

/// Top-level error type for the model registry crate.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The requested model was not found.
    #[error("model not found: {0}")]
    ModelNotFound(String),
    /// A policy operation failed.
    #[error("policy error: {0}")]
    Policy(String),
    /// The model ID could not be parsed.
    #[error("cannot parse model ID: {0}")]
    InvalidModelId(String),
}

/// Crate-level [`Result`] alias.
pub type Result<T> = std::result::Result<T, Error>;
