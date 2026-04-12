use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub use crate::registry::categories::{
    CapabilityCategory, ContextWindowCategory, CostCategory, ProviderCategory, TierCategory,
};

/// Error type for policy loading operations.
#[derive(Debug, thiserror::Error)]
pub enum PolicyLoadError {
    /// I/O error reading policy file.
    #[error("I/O error: {0}")]
    Io(String),
    /// JSON parse error.
    #[error("parse error: {0}")]
    Parse(String),
    /// Schema validation failure.
    #[error("schema validation failed: {0}")]
    Schema(String),
}

/// Routing policy combining multiple dimension filters.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RoutingPolicy {
    /// Unique policy identifier.
    pub id: String,
    /// Human-readable policy name.
    pub name: String,
    /// Policy priority (higher = evaluated first).
    pub priority: i32,
    /// Whether the policy is active.
    pub enabled: bool,
    /// Dimension filters (all must match for the policy to apply).
    pub filters: PolicyFilters,
    /// Action to take when the policy matches.
    pub action: PolicyAction,
    /// Additional conditions for conditional application.
    #[serde(default)]
    pub conditions: Vec<PolicyCondition>,
}

/// Dimension filters applied during policy matching.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct PolicyFilters {
    /// Required capabilities (model must have ALL).
    #[serde(default)]
    pub capabilities: Vec<CapabilityFilter>,
    /// Allowed tiers (model must be in ANY).
    #[serde(default)]
    pub tiers: Vec<TierCategory>,
    /// Allowed cost categories (model must be in ANY).
    #[serde(default)]
    pub costs: Vec<CostCategory>,
    /// Allowed context window categories (model must be in ANY).
    #[serde(default)]
    pub context_windows: Vec<ContextWindowCategory>,
    /// Allowed providers (model must be from ANY).
    #[serde(default)]
    pub providers: Vec<ProviderCategory>,
    /// Required modalities (model must support ALL).
    #[serde(default)]
    pub modalities: Vec<ModalityCategory>,
}

/// Capability filter with match mode.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilityFilter {
    /// Capability to check.
    pub capability: CapabilityCategory,
    /// Match mode: "require" (must have), "prefer" (bonus), "exclude" (must not have).
    #[serde(default = "default_capability_mode")]
    pub mode: String,
}

fn default_capability_mode() -> String {
    "require".to_string()
}

/// Input/output modality categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModalityCategory {
    /// Text input/output.
    Text,
    /// Image input.
    Image,
    /// Audio input/output.
    Audio,
    /// Video input.
    Video,
    /// Embedding output.
    Embedding,
    /// Code generation.
    Code,
}

impl ModalityCategory {
    /// Returns the `snake_case` string representation.
    #[must_use]
    pub const fn as_str(&self) -> &str {
        match self {
            Self::Text => "text",
            Self::Image => "image",
            Self::Audio => "audio",
            Self::Video => "video",
            Self::Embedding => "embedding",
            Self::Code => "code",
        }
    }

    /// Parses a string into a modality category.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "text" => Some(Self::Text),
            "image" => Some(Self::Image),
            "audio" => Some(Self::Audio),
            "video" => Some(Self::Video),
            "embedding" => Some(Self::Embedding),
            "code" => Some(Self::Code),
            _ => None,
        }
    }
}

/// Action to take when a policy matches.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PolicyAction {
    /// Routing strategy: "prefer", "avoid", "block", "weight".
    #[serde(default = "default_action_type")]
    pub action_type: String,
    /// Weight adjustment factor (for "weight" action type).
    #[serde(default)]
    pub weight_factor: f64,
    /// Preferred providers in priority order.
    #[serde(default)]
    pub preferred_providers: Vec<ProviderCategory>,
    /// Preferred model IDs.
    #[serde(default)]
    pub preferred_models: Vec<String>,
    /// Models or providers to avoid.
    #[serde(default)]
    pub avoid: Vec<String>,
    /// Maximum cost per million tokens (soft limit).
    #[serde(default)]
    pub max_cost_per_million: Option<f64>,
    /// Minimum context window required.
    #[serde(default)]
    pub min_context_window: Option<usize>,
}

fn default_action_type() -> String {
    "prefer".to_string()
}

/// Condition for conditional policy application.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyCondition {
    /// Condition type.
    pub condition_type: PolicyConditionType,
    /// Condition value.
    pub value: String,
    /// Comparison operator.
    #[serde(default = "default_operator")]
    pub operator: String,
}

/// Types of conditions that can gate policy application.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PolicyConditionType {
    /// Hour of day (0-23).
    TimeOfDay,
    /// Day of week (0=Sunday).
    DayOfWeek,
    /// Request token count.
    TokenCount,
    /// User/tenant identifier.
    TenantId,
    /// Model family.
    ModelFamily,
    /// Custom metadata field.
    Custom,
}

fn default_operator() -> String {
    "eq".to_string()
}

/// Runtime context evaluated against policy conditions.
#[derive(Debug, Clone, Default)]
pub struct PolicyContext {
    /// Current hour (0-23).
    pub hour_of_day: Option<i32>,
    /// Current day of week (0-6).
    pub day_of_week: Option<i32>,
    /// Estimated token count.
    pub token_count: Option<usize>,
    /// Tenant identifier.
    pub tenant_id: Option<String>,
    /// Model family being requested.
    pub model_family: Option<String>,
    /// Custom key-value metadata.
    pub metadata: HashMap<String, String>,
}

/// Result of a policy evaluation.
#[derive(Debug, Clone)]
pub struct PolicyMatch {
    /// The matched policy.
    pub policy: RoutingPolicy,
    /// Match score (higher = better fit).
    pub score: f64,
    /// Whether all conditions were satisfied.
    pub conditions_met: bool,
}

impl RoutingPolicy {
    /// Creates a new policy with default settings.
    #[must_use]
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            priority: 0,
            enabled: true,
            filters: PolicyFilters::default(),
            action: PolicyAction::default(),
            conditions: Vec::new(),
        }
    }

    /// Sets the policy priority.
    #[must_use]
    pub const fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// Adds a capability filter with the given match mode.
    #[must_use]
    pub fn with_capability(
        mut self,
        capability: CapabilityCategory,
        mode: impl Into<String>,
    ) -> Self {
        self.filters.capabilities.push(CapabilityFilter {
            capability,
            mode: mode.into(),
        });
        self
    }

    /// Adds a tier filter (deduplicated).
    #[must_use]
    pub fn with_tier(mut self, tier: TierCategory) -> Self {
        if !self.filters.tiers.contains(&tier) {
            self.filters.tiers.push(tier);
        }
        self
    }

    /// Adds a provider filter (deduplicated).
    #[must_use]
    pub fn with_provider(mut self, provider: ProviderCategory) -> Self {
        if !self.filters.providers.contains(&provider) {
            self.filters.providers.push(provider);
        }
        self
    }

    /// Sets the action type.
    #[must_use]
    pub fn with_action(mut self, action_type: impl Into<String>) -> Self {
        self.action.action_type = action_type.into();
        self
    }

    /// Sets the weight factor.
    #[must_use]
    pub const fn with_weight_factor(mut self, factor: f64) -> Self {
        self.action.weight_factor = factor;
        self
    }

    /// Returns `true` if the policy is enabled and all conditions are met.
    #[must_use]
    pub fn matches(&self, context: &PolicyContext) -> bool {
        if !self.enabled {
            return false;
        }

        // Check all conditions
        for condition in &self.conditions {
            if !Self::evaluate_condition(condition, context) {
                return false;
            }
        }

        true
    }

    fn evaluate_condition(condition: &PolicyCondition, context: &PolicyContext) -> bool {
        let actual_value = match condition.condition_type {
            PolicyConditionType::TimeOfDay => context.hour_of_day.map(|h| h.to_string()),
            PolicyConditionType::DayOfWeek => context.day_of_week.map(|d| d.to_string()),
            PolicyConditionType::TokenCount => context.token_count.map(|t| t.to_string()),
            PolicyConditionType::TenantId => context.tenant_id.clone(),
            PolicyConditionType::ModelFamily => context.model_family.clone(),
            PolicyConditionType::Custom => {
                // Parse value as "key:value" format
                let parts: Vec<&str> = condition.value.splitn(2, ':').collect();
                if parts.len() == 2 {
                    let key = parts[0];
                    let value = parts[1];
                    if key.is_empty() || value.is_empty() {
                        None
                    } else {
                        context.metadata.get(key).cloned()
                    }
                } else {
                    None
                }
            },
        };

        actual_value.is_some_and(|actual| {
            if condition.condition_type == PolicyConditionType::TokenCount {
                // Numeric comparison to avoid lexicographic issues
                // e.g., "999" > "1000" lexicographically which is wrong for numbers
                let actual_num = actual.parse::<i64>().unwrap_or_else(|_| {
                    tracing::warn!(
                        "Non-numeric TokenCount actual value '{}' in condition, defaulting to 0",
                        actual
                    );
                    0
                });
                let condition_num = condition.value.parse::<i64>().unwrap_or_else(|_| {
                    tracing::warn!(
                        "Non-numeric TokenCount value '{}' in condition, defaulting to 0",
                        condition.value
                    );
                    0
                });
                match condition.operator.as_str() {
                    "eq" | "==" => actual_num == condition_num,
                    "ne" | "!=" => actual_num != condition_num,
                    "gt" | ">" => actual_num > condition_num,
                    "gte" | ">=" => actual_num >= condition_num,
                    "lt" | "<" => actual_num < condition_num,
                    "lte" | "<=" => actual_num <= condition_num,
                    _ => false,
                }
            } else {
                match condition.operator.as_str() {
                    "eq" | "==" => actual == condition.value,
                    "ne" | "!=" => actual != condition.value,
                    "gt" | ">" => actual > condition.value,
                    "gte" | ">=" => actual >= condition.value,
                    "lt" | "<" => actual < condition.value,
                    "lte" | "<=" => actual <= condition.value,
                    "contains" => actual.contains(&condition.value),
                    "in" if !condition.value.is_empty() => {
                        condition.value.split(',').any(|v| v.trim() == actual)
                    },
                    _ => false,
                }
            }
        })
    }
}

impl Default for PolicyAction {
    fn default() -> Self {
        Self {
            action_type: "prefer".to_string(),
            weight_factor: 1.0,
            preferred_providers: Vec::new(),
            preferred_models: Vec::new(),
            avoid: Vec::new(),
            max_cost_per_million: None,
            min_context_window: None,
        }
    }
}
