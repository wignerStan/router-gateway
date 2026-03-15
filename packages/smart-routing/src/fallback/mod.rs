//! Fallback route planning for credential failover.

mod planning;

#[cfg(test)]
mod tests;

/// Fallback route with ordering information
#[derive(Debug, Clone)]
pub struct FallbackRoute {
    /// Auth credential ID for this fallback
    pub auth_id: String,
    /// Position in fallback chain (0 = primary)
    pub position: usize,
    /// Calculated weight for this auth
    pub weight: f64,
    /// Provider identifier (if available)
    pub provider: Option<String>,
}

/// Configuration for fallback planning
#[derive(Debug, Clone)]
pub struct FallbackConfig {
    /// Maximum number of fallback routes to generate
    pub max_fallbacks: usize,
    /// Minimum number of fallbacks (even with limited candidates)
    pub min_fallbacks: usize,
    /// Enable provider diversity in fallback chain
    pub enable_provider_diversity: bool,
    /// Prefer different providers for consecutive fallbacks
    pub prefer_diverse_providers: bool,
}

impl Default for FallbackConfig {
    fn default() -> Self {
        Self {
            max_fallbacks: 5,
            min_fallbacks: 2,
            enable_provider_diversity: true,
            prefer_diverse_providers: true,
        }
    }
}

/// Fallback planner for generating ordered fallback routes
#[derive(Debug, Clone)]
pub struct FallbackPlanner {
    config: FallbackConfig,
}

impl FallbackPlanner {
    /// Create a new fallback planner with default config
    pub fn new() -> Self {
        Self {
            config: FallbackConfig::default(),
        }
    }

    /// Create a new fallback planner with custom config
    pub fn with_config(config: FallbackConfig) -> Self {
        Self { config }
    }

    /// Get config
    pub fn config(&self) -> &FallbackConfig {
        &self.config
    }

    /// Set config
    pub fn set_config(&mut self, config: FallbackConfig) {
        self.config = config;
    }
}

impl Default for FallbackPlanner {
    fn default() -> Self {
        Self::new()
    }
}
