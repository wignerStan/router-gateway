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
    /// Create a new fallback planner with default config.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: FallbackConfig::default(),
        }
    }

    /// Create a new fallback planner with custom config.
    #[must_use]
    pub const fn with_config(config: FallbackConfig) -> Self {
        Self { config }
    }

    /// Get config.
    #[must_use]
    pub const fn config(&self) -> &FallbackConfig {
        &self.config
    }

    /// Set config
    pub const fn set_config(&mut self, config: FallbackConfig) {
        self.config = config;
    }
}

impl Default for FallbackPlanner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod constructor_tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn fallback_config_default_values() {
        let config = FallbackConfig::default();
        assert_eq!(config.max_fallbacks, 5);
        assert_eq!(config.min_fallbacks, 2);
        assert!(config.enable_provider_diversity);
        assert!(config.prefer_diverse_providers);
    }

    #[test]
    fn planner_new_has_default_config() {
        let planner = FallbackPlanner::new();
        assert_eq!(planner.config().max_fallbacks, 5);
        assert_eq!(planner.config().min_fallbacks, 2);
    }

    #[test]
    fn planner_with_custom_config() {
        let config = FallbackConfig {
            max_fallbacks: 10,
            min_fallbacks: 1,
            enable_provider_diversity: false,
            prefer_diverse_providers: false,
        };
        let planner = FallbackPlanner::with_config(config);
        assert_eq!(planner.config().max_fallbacks, 10);
        assert_eq!(planner.config().min_fallbacks, 1);
        assert!(!planner.config().enable_provider_diversity);
    }

    #[test]
    fn planner_set_config_updates_config() {
        let mut planner = FallbackPlanner::new();
        let new_config = FallbackConfig {
            max_fallbacks: 3,
            min_fallbacks: 1,
            enable_provider_diversity: false,
            prefer_diverse_providers: false,
        };
        planner.set_config(new_config);
        assert_eq!(planner.config().max_fallbacks, 3);
    }

    #[test]
    fn planner_default_trait() {
        let planner = FallbackPlanner::default();
        assert_eq!(planner.config().max_fallbacks, 5);
    }
}
