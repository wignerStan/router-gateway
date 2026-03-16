use rand::Rng;
use std::collections::HashMap;

use super::{BanditConfig, BanditPolicy, RouteStats, Tier};

impl BanditPolicy {
    /// Select a route using Thompson sampling
    ///
    /// Returns the selected route ID, or None if no routes available
    pub fn select_route(&self, route_ids: &[&str]) -> Option<String> {
        if route_ids.is_empty() {
            return None;
        }

        if route_ids.len() == 1 {
            return Some(route_ids[0].to_string());
        }

        // Calculate Thompson sample for each route
        let samples: Vec<f64> = route_ids
            .iter()
            .map(|id| self.thompson_sample(id))
            .collect();

        // Select route with highest sample
        let best = best_index(&samples);
        Some(route_ids[best].to_string())
    }

    /// Select route with utility-weighted Thompson sampling
    pub fn select_route_with_utility(
        &self,
        route_ids: &[&str],
        utilities: &HashMap<&str, f64>,
    ) -> Option<String> {
        if route_ids.is_empty() {
            return None;
        }

        if route_ids.len() == 1 {
            return Some(route_ids[0].to_string());
        }

        // Calculate weighted Thompson sample for each route
        let samples: Vec<f64> = route_ids
            .iter()
            .map(|id| {
                let thompson_sample = self.thompson_sample(id);
                let utility = utilities.get(id).copied().unwrap_or(0.5);

                // Weight Thompson sample by utility
                if self.config.use_utility_weighting {
                    thompson_sample * utility
                } else {
                    thompson_sample
                }
            })
            .collect();

        // Select route with highest weighted sample
        let best = best_index(&samples);
        Some(route_ids[best].to_string())
    }

    /// Thompson sampling: draw from Beta(alpha, beta)
    pub(crate) fn thompson_sample(&self, route_id: &str) -> f64 {
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
    pub(crate) fn sample_beta(&self, alpha: f64, beta: f64) -> f64 {
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
    pub(crate) fn sample_gamma(&self, shape: f64) -> f64 {
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
                x = 2.0f64.mul_add(x, -1.0);
                v = 1.0 + c * x;

                if v > 0.0 {
                    break;
                }
            }

            v = v * v * v;
            let u: f64 = rng.gen();

            if u < 0.0331f64.mul_add(-(x * x).powi(2), 1.0) {
                return d * v;
            }

            if (u / v).ln() < (0.5 * x).mul_add(x, d * (1.0 - v + v.ln())) {
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
    pub const fn get_all_stats(&self) -> &HashMap<String, RouteStats> {
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
    pub const fn utility_estimator(&self) -> &crate::utility::UtilityEstimator {
        &self.utility_estimator
    }

    /// Get config
    pub const fn config(&self) -> &BanditConfig {
        &self.config
    }
}

/// Find the index of the maximum value in a slice
fn best_index(samples: &[f64]) -> usize {
    samples
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .map_or(0, |(i, _)| i)
}
