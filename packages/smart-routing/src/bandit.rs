use crate::utility::UtilityEstimator;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
    /// Optional tier-based priors (overrides prior_successes/prior_failures when tier is known)
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
    fn get(&self, tier: &Tier) -> (f64, f64) {
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

    /// Select a route using Thompson sampling
    ///
    /// Returns the selected route ID, or None if no routes available
    pub fn select_route(&self, route_ids: &[String]) -> Option<String> {
        if route_ids.is_empty() {
            return None;
        }

        if route_ids.len() == 1 {
            return Some(route_ids[0].clone());
        }

        // Calculate Thompson sample for each route
        let mut samples: Vec<(String, f64)> = route_ids
            .iter()
            .map(|id| {
                let sample = self.thompson_sample(id);
                (id.clone(), sample)
            })
            .collect();

        // Select route with highest sample (NaN-safe: treat NaN as equal)
        samples.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        Some(samples.remove(0).0)
    }

    /// Select route with utility-weighted Thompson sampling
    pub fn select_route_with_utility(
        &self,
        route_ids: &[String],
        utilities: &HashMap<String, f64>,
    ) -> Option<String> {
        if route_ids.is_empty() {
            return None;
        }

        if route_ids.len() == 1 {
            return Some(route_ids[0].clone());
        }

        // Calculate weighted Thompson sample for each route
        let mut samples: Vec<(String, f64)> = route_ids
            .iter()
            .map(|id| {
                let thompson_sample = self.thompson_sample(id);
                let utility = utilities.get(id).copied().unwrap_or(0.5);

                // Weight Thompson sample by utility
                let weighted_sample = if self.config.use_utility_weighting {
                    thompson_sample * utility
                } else {
                    thompson_sample
                };

                (id.clone(), weighted_sample)
            })
            .collect();

        // Select route with highest weighted sample (NaN-safe: treat NaN as equal)
        samples.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        Some(samples.remove(0).0)
    }

    /// Thompson sampling: draw from Beta(alpha, beta)
    fn thompson_sample(&self, route_id: &str) -> f64 {
        let stats = self.route_stats.get(route_id);

        match stats {
            None => {
                let (alpha, beta) = self.get_prior(route_id);
                self.sample_beta(alpha, beta)
            },
            Some(s) if s.pulls < self.config.min_samples_for_thompson => {
                let (alpha, beta) = self.get_prior(route_id);
                self.sample_beta(alpha, beta)
            },
            Some(s) => {
                // Use actual statistics
                let alpha = s.successes;
                let beta = s.failures;

                // Apply diversity penalty
                let base_sample = self.sample_beta(alpha, beta);
                let penalty = s.diversity_penalty * self.config.diversity_weight;
                (base_sample - penalty).max(0.0)
            },
        }
    }

    /// Sample from Beta distribution using gamma distribution
    fn sample_beta(&self, alpha: f64, beta: f64) -> f64 {
        let mut rng = rand::thread_rng();

        // Beta(alpha, beta) = Gamma(alpha, 1) / (Gamma(alpha, 1) + Gamma(beta, 1))
        // Use approximation for speed: Beta ~ Normal for large parameters

        if alpha > 10.0 && beta > 10.0 {
            // Normal approximation for large parameters
            let mean = alpha / (alpha + beta);
            let variance = (alpha * beta) / ((alpha + beta).powi(2) * (alpha + beta + 1.0));
            let std_dev = variance.sqrt();

            // Box-Muller transform for normal distribution
            let u1: f64 = rng.gen();
            let u2: f64 = rng.gen();
            let z = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();

            (mean + z * std_dev).clamp(0.0, 1.0)
        } else {
            // Gamma sampling for small parameters
            let gamma_alpha = self.sample_gamma(alpha);
            let gamma_beta = self.sample_gamma(beta);
            let sum = gamma_alpha + gamma_beta;

            if sum > 0.0 {
                gamma_alpha / sum
            } else {
                0.5
            }
        }
    }

    /// Sample from Gamma distribution using Marsaglia and Tsang's method
    fn sample_gamma(&self, shape: f64) -> f64 {
        let mut rng = rand::thread_rng();

        if shape < 1.0 {
            // Use transformation for shape < 1
            let u: f64 = rng.gen();
            return self.sample_gamma(1.0 + shape) * u.powf(1.0 / shape);
        }

        let d = shape - 1.0 / 3.0;
        let c = (1.0 / 3.0) / d.sqrt();

        loop {
            let mut x: f64;
            let mut v: f64;

            loop {
                x = rng.gen();
                x = 2.0 * x - 1.0;
                v = 1.0 + c * x;

                if v > 0.0 {
                    break;
                }
            }

            v = v * v * v;
            let u: f64 = rng.gen();

            if u < 1.0 - 0.0331 * (x * x).powi(2) {
                return d * v;
            }

            if (u / v).ln() < 0.5 * x * x + d * (1.0 - v + v.ln()) {
                return d * v;
            }
        }
    }

    /// Record route selection result
    pub fn record_result(&mut self, route_id: &str, success: bool, utility: f64) {
        let stats = self.route_stats.entry(route_id.to_string()).or_default();

        // Update statistics
        if success {
            stats.successes += 1.0;
        } else {
            stats.failures += 1.0;
        }
        stats.pulls += 1;
        stats.last_utility = utility;

        // Decay old samples
        if self.config.sample_decay < 1.0 && stats.pulls > 10 {
            let decay = self.config.sample_decay;
            stats.successes *= decay;
            stats.failures *= decay;
        }
    }

    /// Set diversity penalty for correlated routes
    pub fn set_diversity_penalty(&mut self, route_id: &str, penalty: f64) {
        let stats = self.route_stats.entry(route_id.to_string()).or_default();
        stats.diversity_penalty = penalty.clamp(0.0, 1.0);
    }

    /// Get route statistics
    pub fn get_stats(&self, route_id: &str) -> Option<&RouteStats> {
        self.route_stats.get(route_id)
    }

    /// Get all route statistics
    pub fn get_all_stats(&self) -> &HashMap<String, RouteStats> {
        &self.route_stats
    }

    /// Reset statistics for a route
    pub fn reset_route(&mut self, route_id: &str) {
        self.route_stats.remove(route_id);
    }

    /// Reset all statistics
    pub fn reset_all(&mut self) {
        self.route_stats.clear();
    }

    /// Set the tier for a route (used for tier-based priors)
    pub fn set_route_tier(&mut self, route_id: &str, tier: Tier) {
        self.route_tiers.insert(route_id.to_string(), tier);
    }

    /// Get the prior (alpha, beta) for a route, considering tier if configured
    fn get_prior(&self, route_id: &str) -> (f64, f64) {
        self.route_tiers
            .get(route_id)
            .and_then(|tier| self.config.tier_priors.as_ref().map(|tp| tp.get(tier)))
            .unwrap_or((self.config.prior_successes, self.config.prior_failures))
    }

    /// Get utility estimator
    pub fn utility_estimator(&self) -> &UtilityEstimator {
        &self.utility_estimator
    }

    /// Get config
    pub fn config(&self) -> &BanditConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bandit_select_route_empty() {
        let policy = BanditPolicy::new();
        let result = policy.select_route(&[]);
        assert!(result.is_none());
    }

    #[test]
    fn test_bandit_select_route_single() {
        let policy = BanditPolicy::new();
        let result = policy.select_route(&["route1".to_string()]);
        assert_eq!(result, Some("route1".to_string()));
    }

    #[test]
    fn test_bandit_select_route_multiple() {
        let policy = BanditPolicy::new();
        let routes = vec![
            "route1".to_string(),
            "route2".to_string(),
            "route3".to_string(),
        ];

        let result = policy.select_route(&routes);
        assert!(result.is_some());
        assert!(routes.contains(&result.unwrap()));
    }

    #[test]
    fn test_bandit_record_result() {
        let mut policy = BanditPolicy::new();

        policy.record_result("route1", true, 0.8);
        policy.record_result("route1", true, 0.9);
        policy.record_result("route1", false, 0.3);

        let stats = policy.get_stats("route1").unwrap();
        assert_eq!(stats.successes, 2.0 + 1.0); // 2 successes + prior
        assert_eq!(stats.failures, 1.0 + 1.0); // 1 failure + prior
        assert_eq!(stats.pulls, 3);
        assert_eq!(stats.last_utility, 0.3);
    }

    #[test]
    fn test_bandit_exploration_vs_exploitation() {
        let mut policy = BanditPolicy::new();

        // Train route1 to be good
        for _ in 0..20 {
            policy.record_result("route1", true, 0.9);
        }

        // Train route2 to be bad
        for _ in 0..20 {
            policy.record_result("route2", false, 0.2);
        }

        // route3 is unknown (exploration candidate)

        let routes = vec![
            "route1".to_string(),
            "route2".to_string(),
            "route3".to_string(),
        ];

        // Run many selections and count
        let mut counts = HashMap::new();
        for _ in 0..100 {
            let selected = policy.select_route(&routes).unwrap();
            *counts.entry(selected).or_insert(0) += 1;
        }

        // route1 should be selected most (exploitation)
        // route3 should get some selections (exploration)
        // route2 should be selected least
        assert!(counts.get("route1").unwrap_or(&0) > counts.get("route2").unwrap_or(&0));
    }

    #[test]
    fn test_bandit_diversity_penalty() {
        // Use a higher diversity_weight and lower min_samples to ensure penalty is applied
        let config = BanditConfig {
            diversity_weight: 0.5,       // Higher weight for more pronounced penalty effect
            min_samples_for_thompson: 1, // Ensure penalty is applied after first result
            ..Default::default()
        };
        let mut policy = BanditPolicy::with_config(config);

        // Record enough results to exceed min_samples_for_thompson
        for _ in 0..5 {
            policy.record_result("route1", true, 0.95);
            policy.record_result("route2", true, 0.95);
        }

        // Set high diversity penalty on route2
        policy.set_diversity_penalty("route2", 1.0);

        // route1 should be selected more often due to penalty on route2
        let routes = vec!["route1".to_string(), "route2".to_string()];

        let mut count1 = 0;
        // Use larger sample size for statistical significance
        for _ in 0..500 {
            if policy.select_route(&routes) == Some("route1".to_string()) {
                count1 += 1;
            }
        }

        // With penalty of 1.0 and diversity_weight of 0.5, route2 gets a 0.5 penalty
        // This should give route1 a measurable advantage (>55% selection rate)
        assert!(
            count1 > 275,
            "route1 selected {} out of 500 times, expected > 275",
            count1
        );
    }

    #[test]
    fn test_bandit_utility_weighting() {
        let mut policy = BanditPolicy::new();

        policy.record_result("route1", true, 0.9);
        policy.record_result("route2", true, 0.9);

        let mut utilities = HashMap::new();
        utilities.insert("route1".to_string(), 0.2); // Low utility
        utilities.insert("route2".to_string(), 0.9); // High utility

        let routes = vec!["route1".to_string(), "route2".to_string()];

        let mut count2 = 0;
        for _ in 0..50 {
            if policy.select_route_with_utility(&routes, &utilities) == Some("route2".to_string()) {
                count2 += 1;
            }
        }

        // route2 should be selected more due to higher utility
        assert!(count2 > 25);
    }

    #[test]
    fn test_bandit_reset_route() {
        let mut policy = BanditPolicy::new();

        policy.record_result("route1", true, 0.9);
        assert!(policy.get_stats("route1").is_some());

        policy.reset_route("route1");
        assert!(policy.get_stats("route1").is_none());
    }

    #[test]
    fn test_bandit_reset_all() {
        let mut policy = BanditPolicy::new();

        policy.record_result("route1", true, 0.9);
        policy.record_result("route2", false, 0.3);

        assert_eq!(policy.get_all_stats().len(), 2);

        policy.reset_all();
        assert_eq!(policy.get_all_stats().len(), 0);
    }

    #[test]
    fn test_beta_sampling_bounds() {
        let policy = BanditPolicy::new();

        // Test that beta samples are in [0, 1]
        for _ in 0..100 {
            let sample = policy.sample_beta(1.0, 1.0);
            assert!((0.0..=1.0).contains(&sample));
        }
    }

    #[test]
    fn test_beta_sampling_distribution() {
        let policy = BanditPolicy::new();

        // Beta(1, 1) should be uniform around 0.5
        let mut sum = 0.0;
        for _ in 0..1000 {
            sum += policy.sample_beta(1.0, 1.0);
        }
        let mean = sum / 1000.0;

        // Mean should be close to 0.5
        assert!((mean - 0.5).abs() < 0.1);
    }

    #[test]
    fn test_gamma_sampling_positive() {
        let policy = BanditPolicy::new();

        // Test that gamma samples are positive
        for _ in 0..100 {
            let sample = policy.sample_gamma(2.0);
            assert!(sample > 0.0);
        }
    }

    #[test]
    fn test_prior_initialization() {
        let policy = BanditPolicy::new();

        // Unknown route should use optimistic prior
        let routes = vec!["unknown".to_string()];
        let result = policy.select_route(&routes);
        assert_eq!(result, Some("unknown".to_string()));
    }

    #[test]
    fn test_sample_decay() {
        let config = BanditConfig {
            sample_decay: 0.9,
            ..Default::default()
        };

        let mut policy = BanditPolicy::with_config(config);

        policy.record_result("route1", true, 0.9);
        policy.record_result("route1", true, 0.9);

        let stats1 = policy.get_stats("route1").unwrap();
        let successes_after_2 = stats1.successes;

        // Add more pulls
        for _ in 0..10 {
            policy.record_result("route1", true, 0.9);
        }

        let stats2 = policy.get_stats("route1").unwrap();

        // With decay, successes should not grow linearly
        assert!(stats2.successes < successes_after_2 + 10.0);
    }

    // ============================================================
    // Edge Case Tests for BanditPolicy - Thompson Sampling
    // ============================================================

    #[test]
    fn test_beta_sampling_very_small_alpha_beta() {
        let policy = BanditPolicy::new();

        // Very small parameters (near zero)
        for _ in 0..100 {
            let sample = policy.sample_beta(0.001, 0.001);
            assert!(
                (0.0..=1.0).contains(&sample),
                "Sample should be in [0,1] with small params: {}",
                sample
            );
        }
    }

    #[test]
    fn test_beta_sampling_very_large_alpha_beta() {
        let policy = BanditPolicy::new();

        // Very large parameters (uses normal approximation)
        let mut samples = Vec::new();
        for _ in 0..100 {
            let sample = policy.sample_beta(100.0, 100.0);
            assert!(
                (0.0..=1.0).contains(&sample),
                "Sample should be in [0,1] with large params: {}",
                sample
            );
            samples.push(sample);
        }

        // Mean should be close to alpha / (alpha + beta) = 0.5
        let mean: f64 = samples.iter().sum::<f64>() / samples.len() as f64;
        assert!(
            (mean - 0.5).abs() < 0.1,
            "Mean should be close to 0.5 with large symmetric params: {}",
            mean
        );
    }

    #[test]
    fn test_gamma_sampling_shape_less_than_one() {
        let policy = BanditPolicy::new();

        // Shape < 1 uses transformation method
        for shape in [0.1, 0.5, 0.9].iter() {
            for _ in 0..50 {
                let sample = policy.sample_gamma(*shape);
                assert!(
                    sample > 0.0,
                    "Gamma sample with shape {} should be positive: {}",
                    shape,
                    sample
                );
                assert!(
                    sample.is_finite(),
                    "Gamma sample with shape {} should be finite: {}",
                    shape,
                    sample
                );
            }
        }
    }

    #[test]
    fn test_diversity_penalty_clamping() {
        let mut policy = BanditPolicy::new();

        // Test clamping to [0, 1]
        policy.set_diversity_penalty("route1", -0.5);
        let stats = policy.get_stats("route1").unwrap();
        assert_eq!(
            stats.diversity_penalty, 0.0,
            "Negative penalty should be clamped to 0"
        );

        policy.set_diversity_penalty("route1", 1.5);
        let stats = policy.get_stats("route1").unwrap();
        assert_eq!(
            stats.diversity_penalty, 1.0,
            "Penalty > 1 should be clamped to 1"
        );

        policy.set_diversity_penalty("route1", 0.5);
        let stats = policy.get_stats("route1").unwrap();
        assert_eq!(
            stats.diversity_penalty, 0.5,
            "Penalty in [0,1] should be unchanged"
        );
    }

    #[test]
    fn test_numerical_stability_extreme_utility_values() {
        let mut policy = BanditPolicy::new();

        // Record with extreme utility values
        policy.record_result("route1", true, f64::MAX / 2.0);
        policy.record_result("route2", true, f64::MIN_POSITIVE);
        policy.record_result("route3", true, 0.0);
        policy.record_result("route4", false, f64::INFINITY);
        policy.record_result("route5", false, f64::NAN);

        // Should not panic and should handle gracefully
        let routes = vec![
            "route1".to_string(),
            "route2".to_string(),
            "route3".to_string(),
            "route4".to_string(),
            "route5".to_string(),
        ];

        // Run selections - should not panic
        for _ in 0..50 {
            let result = policy.select_route(&routes);
            assert!(result.is_some());
        }
    }

    #[test]
    fn test_beta_sampling_skewed_distributions() {
        let policy = BanditPolicy::new();

        // Beta(100, 1) should give values close to 1
        let mut samples_high = Vec::new();
        for _ in 0..100 {
            samples_high.push(policy.sample_beta(100.0, 1.0));
        }
        let mean_high: f64 = samples_high.iter().sum::<f64>() / samples_high.len() as f64;
        assert!(
            mean_high > 0.9,
            "Beta(100,1) mean should be high: {}",
            mean_high
        );

        // Beta(1, 100) should give values close to 0
        let mut samples_low = Vec::new();
        for _ in 0..100 {
            samples_low.push(policy.sample_beta(1.0, 100.0));
        }
        let mean_low: f64 = samples_low.iter().sum::<f64>() / samples_low.len() as f64;
        assert!(
            mean_low < 0.1,
            "Beta(1,100) mean should be low: {}",
            mean_low
        );
    }

    #[test]
    fn test_select_route_with_empty_utilities_map() {
        let policy = BanditPolicy::new();
        let routes = vec!["route1".to_string(), "route2".to_string()];

        // Empty utilities map
        let utilities = HashMap::new();

        // Should still work, using default utility
        for _ in 0..20 {
            let result = policy.select_route_with_utility(&routes, &utilities);
            assert!(result.is_some());
            assert!(routes.contains(&result.unwrap()));
        }
    }

    #[test]
    fn test_record_result_with_zero_utility() {
        let mut policy = BanditPolicy::new();

        policy.record_result("route1", true, 0.0);
        let stats = policy.get_stats("route1").unwrap();

        assert_eq!(stats.last_utility, 0.0);
        assert_eq!(stats.successes, 2.0); // 1 + prior
        assert_eq!(stats.pulls, 1);
    }

    #[test]
    fn test_thompson_sample_with_zero_pulls() {
        let policy = BanditPolicy::new();

        // Unknown route should use prior
        let sample = policy.thompson_sample("unknown-route");

        // With prior (1,1), sample should be in [0,1]
        assert!(
            (0.0..=1.0).contains(&sample),
            "Thompson sample for unknown route should use prior"
        );
    }

    #[test]
    fn test_bandit_with_custom_prior() {
        let config = BanditConfig {
            prior_successes: 10.0,
            prior_failures: 2.0,
            min_samples_for_thompson: 100, // Always use prior
            ..Default::default()
        };
        let mut policy = BanditPolicy::with_config(config);

        // Record some results but not enough to exceed min_samples
        policy.record_result("route1", true, 0.9);
        policy.record_result("route1", true, 0.9);

        // Should still use optimistic prior (10,2)
        let mut samples = Vec::new();
        for _ in 0..100 {
            samples.push(policy.thompson_sample("route1"));
        }

        let mean: f64 = samples.iter().sum::<f64>() / samples.len() as f64;
        // Prior mean is 10/(10+2) = 0.833
        assert!(
            mean > 0.7,
            "Mean with optimistic prior (10,2) should be high: {}",
            mean
        );
    }

    // ============================================================
    // Tier-based Priors Tests
    // ============================================================

    #[test]
    fn test_tier_priors_flagship_higher_than_fast() {
        // Flagship tier should have higher mean prior than fast tier
        let tier_priors = TierPriors::default();
        let (f_alpha, f_beta) = tier_priors.flagship;
        let (s_alpha, s_beta) = tier_priors.fast;

        let flagship_mean = f_alpha / (f_alpha + f_beta);
        let fast_mean = s_alpha / (s_alpha + s_beta);

        assert!(
            flagship_mean > fast_mean,
            "Flagship prior mean ({}) should be > fast prior mean ({})",
            flagship_mean,
            fast_mean
        );
    }

    #[test]
    fn test_tier_priors_distribution_shapes_differ() {
        let config = BanditConfig {
            tier_priors: Some(TierPriors::default()),
            min_samples_for_thompson: 100, // Always use priors
            ..Default::default()
        };
        let mut policy = BanditPolicy::with_config(config);

        policy.set_route_tier("flagship-route", Tier::Flagship);
        policy.set_route_tier("fast-route", Tier::Fast);

        // Sample many times and compare means
        let flagship_samples: Vec<f64> = (0..500)
            .map(|_| policy.thompson_sample("flagship-route"))
            .collect();
        let fast_samples: Vec<f64> = (0..500)
            .map(|_| policy.thompson_sample("fast-route"))
            .collect();

        let flagship_mean: f64 =
            flagship_samples.iter().sum::<f64>() / flagship_samples.len() as f64;
        let fast_mean: f64 = fast_samples.iter().sum::<f64>() / fast_samples.len() as f64;

        // Flagship should have significantly higher samples than fast
        assert!(
            flagship_mean > fast_mean + 0.2,
            "Flagship mean ({}) should be at least 0.2 higher than fast mean ({})",
            flagship_mean,
            fast_mean
        );
    }

    #[test]
    fn test_tier_priors_none_uses_default() {
        // Without tier_priors configured, all routes use default priors
        let config = BanditConfig {
            tier_priors: None,
            prior_successes: 3.0,
            prior_failures: 1.0,
            min_samples_for_thompson: 100,
            ..Default::default()
        };
        let mut policy = BanditPolicy::with_config(config);

        policy.set_route_tier("flagship-route", Tier::Flagship);
        policy.set_route_tier("fast-route", Tier::Fast);

        // Both should sample from Beta(3,1) regardless of tier
        let flagship_samples: Vec<f64> = (0..200)
            .map(|_| policy.thompson_sample("flagship-route"))
            .collect();
        let fast_samples: Vec<f64> = (0..200)
            .map(|_| policy.thompson_sample("fast-route"))
            .collect();

        let flagship_mean: f64 =
            flagship_samples.iter().sum::<f64>() / flagship_samples.len() as f64;
        let fast_mean: f64 = fast_samples.iter().sum::<f64>() / fast_samples.len() as f64;

        // Both means should be close to 3/(3+1) = 0.75
        assert!(
            (flagship_mean - 0.75).abs() < 0.15,
            "Flagship mean ({}) should be near 0.75 without tier priors",
            flagship_mean
        );
        assert!(
            (fast_mean - 0.75).abs() < 0.15,
            "Fast mean ({}) should be near 0.75 without tier priors",
            fast_mean
        );
    }

    #[test]
    fn test_tier_priors_no_tier_set_uses_default() {
        // Route without tier set falls back to default priors
        let config = BanditConfig {
            tier_priors: Some(TierPriors::default()),
            prior_successes: 2.0,
            prior_failures: 2.0,
            min_samples_for_thompson: 100,
            ..Default::default()
        };
        let policy = BanditPolicy::with_config(config);

        // "unknown-tier-route" has no tier set
        let samples: Vec<f64> = (0..200)
            .map(|_| policy.thompson_sample("unknown-tier-route"))
            .collect();
        let mean: f64 = samples.iter().sum::<f64>() / samples.len() as f64;

        // Should use default prior (2,2) -> mean = 0.5
        assert!(
            (mean - 0.5).abs() < 0.15,
            "Untiered route mean ({}) should be near 0.5 (default prior)",
            mean
        );
    }

    #[test]
    fn test_tier_priors_after_enough_samples_uses_stats() {
        // After enough samples, actual stats override priors
        let config = BanditConfig {
            tier_priors: Some(TierPriors::default()),
            min_samples_for_thompson: 3,
            ..Default::default()
        };
        let mut policy = BanditPolicy::with_config(config);

        policy.set_route_tier("flagship-route", Tier::Flagship);

        // Record failures only (contradicts flagship's optimistic prior)
        for _ in 0..10 {
            policy.record_result("flagship-route", false, 0.1);
        }

        let samples: Vec<f64> = (0..200)
            .map(|_| policy.thompson_sample("flagship-route"))
            .collect();
        let mean: f64 = samples.iter().sum::<f64>() / samples.len() as f64;

        // After many failures, samples should be low despite flagship prior
        assert!(
            mean < 0.5,
            "After many failures, mean ({}) should be low despite flagship prior",
            mean
        );
    }
}
