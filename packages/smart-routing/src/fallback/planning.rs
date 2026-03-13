//! Fallback route planning logic.

use crate::health::HealthManager;
use crate::metrics::MetricsCollector;
use crate::weight::{AuthInfo, WeightCalculator};
use std::collections::HashSet;

use super::{FallbackPlanner, FallbackRoute};

/// Internal struct for weighted auth with provider info
#[derive(Debug, Clone)]
struct WeightedAuth {
    id: String,
    weight: f64,
    provider: Option<String>,
}

/// Known provider identifiers, ordered by length (longest first) for greedy matching.
const KNOWN_PROVIDERS: &[&str] = &[
    "azure-openai",
    "amazon-bedrock",
    "byte-dance",
    "google-deepmind",
    "alibaba-cloud",
    "deepseek",
    "openai",
    "google",
    "chrome",
    "xai",
    "mistral",
    "cohere",
    "perplexity",
    "zhipu",
    "baidu",
    "moonshot",
    "meta",
    "azure",
    "bedrock",
    "alibaba",
    "qwen",
    "kimi",
    "grok",
];

impl FallbackPlanner {
    /// Generate ordered fallback routes from available auths
    ///
    /// Returns a Vec of FallbackRoute ordered by:
    /// 1. Weight (descending) - higher weight = better candidate
    /// 2. Provider diversity (if enabled) - prefer different providers
    /// 3. Health status - prefer healthy > degraded > unhealthy
    ///
    /// # Arguments
    /// * `auths` - Available auth credentials
    /// * `primary_id` - The primary selected auth ID (will be first in list)
    /// * `calculator` - Weight calculator for scoring
    /// * `metrics` - Metrics collector for performance data
    /// * `health` - Health manager for status data
    pub async fn generate_fallbacks(
        &self,
        auths: Vec<AuthInfo>,
        primary_id: Option<String>,
        calculator: &dyn WeightCalculator,
        metrics: &MetricsCollector,
        health: &HealthManager,
    ) -> Vec<FallbackRoute> {
        if auths.is_empty() {
            return Vec::new();
        }

        // Calculate weights for all available auths
        let mut weighted_auths = self
            .calculate_weights(auths, calculator, metrics, health)
            .await;

        // Filter to only available (positive weight) auths
        weighted_auths.retain(|w| w.weight > 0.0);

        if weighted_auths.is_empty() {
            return Vec::new();
        }

        // Sort by weight (descending) - highest weight first (NaN-safe)
        weighted_auths.sort_by(|a, b| {
            b.weight
                .partial_cmp(&a.weight)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Apply provider diversity if enabled
        if self.config.enable_provider_diversity {
            weighted_auths = self.apply_provider_diversity(weighted_auths);
        }

        // Ensure primary is first (if specified)
        let fallbacks = if let Some(primary) = primary_id {
            self.ensure_primary_first(weighted_auths, &primary)
        } else {
            weighted_auths
        };

        // Limit to max_fallbacks
        let fallbacks: Vec<_> = fallbacks
            .into_iter()
            .enumerate()
            .map(|(idx, w)| FallbackRoute {
                auth_id: w.id,
                position: idx,
                weight: w.weight,
                provider: w.provider,
            })
            .take(self.config.max_fallbacks)
            .collect();

        // Ensure min_fallbacks by padding with remaining candidates if needed
        if fallbacks.len() < self.config.min_fallbacks {
            // We already filtered by weight > 0, so if we don't have enough,
            // we return what we have (can't invent auths out of thin air)
        }

        fallbacks
    }

    /// Calculate weights for all auths
    async fn calculate_weights(
        &self,
        auths: Vec<AuthInfo>,
        calculator: &dyn WeightCalculator,
        metrics: &MetricsCollector,
        health: &HealthManager,
    ) -> Vec<WeightedAuth> {
        let mut weighted_auths = Vec::new();

        for auth in auths {
            // Skip unavailable auths
            if auth.unavailable {
                continue;
            }

            // Get metrics and health
            let auth_metrics = metrics.get_metrics(&auth.id).await;
            let auth_health = health.get_status(&auth.id).await;

            // Check availability
            let is_available = health.is_available(&auth.id).await;
            if !is_available {
                continue;
            }

            // Calculate weight
            let weight = calculator.calculate(&auth, auth_metrics.as_ref(), auth_health);

            // Extract provider from auth ID (format: "provider-key" or "provider-name-key")
            let provider = self.extract_provider(&auth.id);

            weighted_auths.push(WeightedAuth {
                id: auth.id,
                weight,
                provider,
            });
        }

        weighted_auths
    }

    /// Apply provider diversity to favor different providers
    fn apply_provider_diversity(&self, weighted_auths: Vec<WeightedAuth>) -> Vec<WeightedAuth> {
        if !self.config.prefer_diverse_providers {
            return weighted_auths;
        }

        let mut diversified = Vec::new();
        let mut used_providers: HashSet<String> = HashSet::new();
        let mut remaining = Vec::new();

        // First pass: pick highest weighted auth from each provider
        for auth in weighted_auths.into_iter() {
            if let Some(ref provider) = auth.provider {
                if !used_providers.contains(provider) {
                    used_providers.insert(provider.clone());
                    diversified.push(auth);
                } else {
                    remaining.push(auth);
                }
            } else {
                // No provider info, add to remaining
                remaining.push(auth);
            }
        }

        // Second pass: add remaining auths (might duplicate providers)
        diversified.extend(remaining);

        diversified
    }

    /// Ensure the primary auth is first in the list
    fn ensure_primary_first(
        &self,
        mut auths: Vec<WeightedAuth>,
        primary_id: &str,
    ) -> Vec<WeightedAuth> {
        // Find and remove primary
        let primary_pos = auths.iter().position(|w| w.id == primary_id);

        if let Some(pos) = primary_pos {
            let primary = auths.remove(pos);
            auths.insert(0, primary);
        }

        auths
    }

    /// Extract provider from auth ID using known provider patterns.
    ///
    /// Matches against a known provider list (longest prefix first).
    /// Falls back to first-segment heuristic for unrecognized providers.
    pub(crate) fn extract_provider(&self, auth_id: &str) -> Option<String> {
        if auth_id.is_empty() {
            return None;
        }

        let lower = auth_id.to_lowercase();

        // Try known providers (longest first for greedy match)
        for provider in KNOWN_PROVIDERS {
            if let Some(after) = lower.strip_prefix(provider) {
                // Ensure the match ends at a delimiter or end of string
                if after.is_empty()
                    || after.starts_with('-')
                    || after.starts_with('_')
                    || after.starts_with(':')
                {
                    return Some(provider.to_string());
                }
            }
        }

        // Fallback: first segment
        auth_id
            .split(&['-', '_', ':'])
            .next()
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    }
}
