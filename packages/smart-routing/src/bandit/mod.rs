//! Thompson sampling bandit policy for route selection
//!
//! Balances exploration (trying uncertain routes) vs exploitation (using known good routes).
//! Uses Beta distribution for modeling success/failure outcomes.

use crate::utility::UtilityEstimator;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

mod exploration;

#[cfg(test)]
mod tests;

/// Thompson sampling bandit policy for route selection
///
/// Balances exploration (trying uncertain routes) vs exploitation (using known good routes).
/// Uses Beta distribution for modeling success/failure outcomes.
#[derive(Debug, Clone)]
pub struct BanditPolicy {
    config: BanditConfig,
    /// Route statistics: (successes, failures, pulls)
    route_stats: HashMap<String, RouteStats>,
    /// Route tier mapping
    route_tiers: HashMap<String, Tier>,
    /// Utility estimator for combining metrics
    utility_estimator: UtilityEstimator,
}

/// Route statistics for bandit algorithm
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteStats {
    /// Number of successful selections
    pub successes: f64,
    /// Number of failed selections
    pub failures: f64,
    /// Total number of times this route was selected
    pub pulls: u64,
    /// Last utility estimate
    pub last_utility: f64,
    /// Diversity penalty (for correlated routes)
    pub diversity_penalty: f64,
}

impl Default for RouteStats {
    fn default() -> Self {
        Self {
            successes: 1.0, // Optimistic prior
            failures: 1.0,  // Neutral prior
            pulls: 0,
            last_utility: 0.5,
            diversity_penalty: 0.0,
        }
    }
}

/// Bandit policy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BanditConfig {
    /// Exploration parameter (0-1): higher = more exploration
    pub exploration_rate: f64,
    /// Prior successes for new routes (optimistic initialization)
    pub prior_successes: f64,
    /// Prior failures for new routes
    pub prior_failures: f64,
    /// Diversity penalty weight (0-1)
    pub diversity_weight: f64,
    /// Minimum samples before using Thompson sampling
    pub min_samples_for_thompson: u64,
    /// Whether to use utility-weighted Thompson sampling
    pub use_utility_weighting: bool,
    /// Decay factor for old samples (0-1)
    pub sample_decay: f64,
    /// Optional tier-based priors (overrides `prior_successes/prior_failures` when tier is known)
    pub tier_priors: Option<TierPriors>,
}

impl Default for BanditConfig {
    fn default() -> Self {
        Self {
            exploration_rate: 0.3,
            prior_successes: 1.0,
            prior_failures: 1.0,
            diversity_weight: 0.1,
            min_samples_for_thompson: 5,
            use_utility_weighting: true,
            sample_decay: 0.99,
            tier_priors: None,
        }
    }
}

/// Model tier for prior configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum Tier {
    /// Flagship models (highest quality)
    Flagship,
    /// Standard models
    #[default]
    Standard,
    /// Fast models (lowest latency)
    Fast,
}

/// Tier-specific prior configuration for Thompson sampling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierPriors {
    /// (alpha, beta) for flagship tier - higher prior = more optimistic
    pub flagship: (f64, f64),
    /// (alpha, beta) for standard tier
    pub standard: (f64, f64),
    /// (alpha, beta) for fast tier - lower prior = less optimistic
    pub fast: (f64, f64),
}

impl Default for TierPriors {
    fn default() -> Self {
        Self {
            flagship: (5.0, 1.0), // Highly optimistic
            standard: (2.0, 2.0), // Neutral
            fast: (1.0, 3.0),     // Slightly pessimistic (favor speed over quality)
        }
    }
}

impl TierPriors {
    const fn get(&self, tier: &Tier) -> (f64, f64) {
        match tier {
            Tier::Flagship => self.flagship,
            Tier::Standard => self.standard,
            Tier::Fast => self.fast,
        }
    }
}

impl Default for BanditPolicy {
    fn default() -> Self {
        Self::new()
    }
}

impl BanditPolicy {
    /// Create a new bandit policy with default config
    pub fn new() -> Self {
        Self {
            config: BanditConfig::default(),
            route_stats: HashMap::new(),
            route_tiers: HashMap::new(),
            utility_estimator: UtilityEstimator::new(),
        }
    }

    /// Create a new bandit policy with custom config
    pub fn with_config(config: BanditConfig) -> Self {
        Self {
            config,
            route_stats: HashMap::new(),
            route_tiers: HashMap::new(),
            utility_estimator: UtilityEstimator::new(),
        }
    }

    /// Create a new bandit policy with utility estimator
    pub fn with_utility_estimator(
        config: BanditConfig,
        utility_estimator: UtilityEstimator,
    ) -> Self {
        Self {
            config,
            route_stats: HashMap::new(),
            route_tiers: HashMap::new(),
            utility_estimator,
        }
    }
}
