//! Policy matching engine for evaluating models against multi-dimensional policies
//!
//! The `PolicyMatcher` evaluates whether a `ModelInfo` matches the filters
//! defined in `RoutingPolicy` across all dimensions (capabilities, tier, cost,
//! context window, provider, modalities).

use super::registry::PolicyRegistry;
use super::types::{
    CapabilityCategory, ModalityCategory, PolicyContext, PolicyMatch, RoutingPolicy,
};
use crate::categories::ModelCategorization;
use crate::info::ModelInfo;

/// Evaluates routing policies against model metadata.
pub struct PolicyMatcher {
    registry: PolicyRegistry,
}

impl PolicyMatcher {
    /// Create a new policy matcher with the given registry
    pub const fn new(registry: PolicyRegistry) -> Self {
        Self { registry }
    }

    /// Create a matcher with an empty registry
    pub const fn empty() -> Self {
        Self {
            registry: PolicyRegistry::new(),
        }
    }

    /// Get reference to the underlying registry
    pub const fn registry(&self) -> &PolicyRegistry {
        &self.registry
    }

    /// Get mutable reference to the underlying registry
    pub const fn registry_mut(&mut self) -> &mut PolicyRegistry {
        &mut self.registry
    }

    /// Evaluate all policies against a model in the given context
    ///
    /// Returns a list of `PolicyMatch` for all policies that match the model
    /// and satisfy their conditions.
    pub fn evaluate(&self, model: &ModelInfo, context: &PolicyContext) -> Vec<PolicyMatch> {
        self.registry
            .all()
            .iter()
            .filter(|policy| policy.enabled && policy.matches(context))
            .filter(|policy| self.matches_model(policy, model))
            .map(|policy| {
                let score = self.calculate_score(policy, model, context);
                PolicyMatch {
                    policy: policy.clone(),
                    score,
                    conditions_met: true,
                }
            })
            .collect()
    }

    /// Evaluate and return only the best matching policy (highest priority + score)
    pub fn evaluate_best(&self, model: &ModelInfo, context: &PolicyContext) -> Option<PolicyMatch> {
        let mut matches = self.evaluate(model, context);
        if matches.is_empty() {
            return None;
        }

        // Sort by priority (desc) then score (desc)
        matches.sort_by(|a, b| {
            b.policy.priority.cmp(&a.policy.priority).then(
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
        });

        matches.into_iter().next()
    }

    /// Check if a single policy matches a model's dimensions
    fn matches_model(&self, policy: &RoutingPolicy, model: &ModelInfo) -> bool {
        // Check capabilities (ALL "require" must match, ANY "exclude" must not match)
        for cap_filter in &policy.filters.capabilities {
            let has_capability = match cap_filter.capability {
                CapabilityCategory::Vision => model.capabilities.vision,
                CapabilityCategory::Tools => model.capabilities.tools,
                CapabilityCategory::Streaming => model.capabilities.streaming,
                CapabilityCategory::Thinking => model.capabilities.thinking,
            };

            match cap_filter.mode.as_str() {
                "require" => {
                    if !has_capability {
                        return false;
                    }
                },
                "exclude" => {
                    if has_capability {
                        return false;
                    }
                },
                "prefer" => {
                    // Prefer doesn't block matching, just affects score
                },
                _ => {},
            }
        }

        // Check tier (ANY must match if specified)
        if !policy.filters.tiers.is_empty() && !policy.filters.tiers.contains(&model.get_tier()) {
            return false;
        }

        // Check cost (ANY must match if specified)
        if !policy.filters.costs.is_empty()
            && !policy.filters.costs.contains(&model.get_cost_category())
        {
            return false;
        }

        // Check context window (ANY must match if specified)
        if !policy.filters.context_windows.is_empty()
            && !policy
                .filters
                .context_windows
                .contains(&model.get_context_category())
        {
            return false;
        }

        // Check provider (ANY must match if specified)
        if !policy.filters.providers.is_empty()
            && !policy
                .filters
                .providers
                .contains(&model.get_provider_category())
        {
            return false;
        }

        // Check modalities (ALL must match if specified)
        // Note: ModelInfo doesn't have explicit modality field, so we infer from capabilities
        for modality in &policy.filters.modalities {
            if !self.check_modality_match(model, modality) {
                return false;
            }
        }

        // Check action constraints
        if let Some(max_cost) = policy.action.max_cost_per_million {
            if model.input_price_per_million > max_cost {
                return false;
            }
        }

        if let Some(min_context) = policy.action.min_context_window {
            if model.context_window < min_context {
                return false;
            }
        }

        // Check avoid list
        if policy
            .action
            .avoid
            .iter()
            .any(|avoid_id| model.id.contains(avoid_id) || model.provider.contains(avoid_id))
        {
            return false;
        }

        true
    }

    /// Check if model supports a modality
    const fn check_modality_match(&self, model: &ModelInfo, modality: &ModalityCategory) -> bool {
        match modality {
            ModalityCategory::Text => true, // All models support text
            ModalityCategory::Image => model.capabilities.vision,
            ModalityCategory::Audio => false, // Not tracked in current ModelInfo
            ModalityCategory::Video => model.capabilities.vision, // Vision models often support video
            ModalityCategory::Embedding => false,                 // Would need model type field
            ModalityCategory::Code => true, // Assume all models can generate code
        }
    }

    /// Calculate match score for a policy-model pair
    fn calculate_score(
        &self,
        policy: &RoutingPolicy,
        model: &ModelInfo,
        _context: &PolicyContext,
    ) -> f64 {
        // Base score from action type
        let mut score = match policy.action.action_type.as_str() {
            "prefer" => 1.5,
            "avoid" => 0.5,
            "block" => 0.0,
            "weight" => policy.action.weight_factor,
            _ => 1.0,
        };

        // Bonus for preferred providers
        if policy
            .action
            .preferred_providers
            .contains(&model.get_provider_category())
        {
            score *= 1.2;
        }

        // Bonus for preferred models
        if policy
            .action
            .preferred_models
            .iter()
            .any(|m| model.id.contains(m))
        {
            score *= 1.3;
        }

        // Bonus for "prefer" capability matches
        for cap_filter in &policy.filters.capabilities {
            if cap_filter.mode == "prefer" {
                let has_capability = match cap_filter.capability {
                    CapabilityCategory::Vision => model.capabilities.vision,
                    CapabilityCategory::Tools => model.capabilities.tools,
                    CapabilityCategory::Streaming => model.capabilities.streaming,
                    CapabilityCategory::Thinking => model.capabilities.thinking,
                };
                if has_capability {
                    score *= 1.1;
                }
            }
        }

        // Priority factor (higher priority = higher score)
        let priority_factor = f64::from(policy.priority).mul_add(0.01, 1.0);
        score *= priority_factor;

        score
    }

    /// Calculate combined policy weight factor for a model
    ///
    /// This combines all matching policies' scores into a single weight factor
    /// that can be applied to the base weight calculation.
    pub fn calculate_weight_factor(&self, model: &ModelInfo, context: &PolicyContext) -> f64 {
        let matches = self.evaluate(model, context);

        if matches.is_empty() {
            return 1.0; // No policies match, neutral weight
        }

        // Combine scores from all matching policies
        // Use weighted average based on priority
        let mut total_weight = 0.0;
        let mut total_priority = 0;

        for m in &matches {
            let priority_weight = f64::from(m.policy.priority).mul_add(0.1, 1.0);
            total_weight += m.score * priority_weight;
            total_priority += m.policy.priority;
        }

        if total_priority == 0 {
            return 1.0;
        }

        // Normalize to reasonable range [0.1, 10.0]
        let avg_weight = total_weight / (matches.len() as f64);
        avg_weight.clamp(0.1, 10.0)
    }

    /// Check if a model should be blocked by any policy
    pub fn is_blocked(&self, model: &ModelInfo, context: &PolicyContext) -> bool {
        self.registry.all().iter().any(|policy| {
            policy.enabled
                && policy.matches(context)
                && policy.action.action_type == "block"
                && self.matches_model(policy, model)
        })
    }
}

impl Clone for PolicyMatcher {
    fn clone(&self) -> Self {
        Self {
            registry: self.registry.clone(),
        }
    }
}
