use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub use crate::categories::{
    CapabilityCategory, ContextWindowCategory, CostCategory, ProviderCategory, TierCategory,
};

/// Routing policy that combines multiple dimension filters
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RoutingPolicy {
    /// Unique policy identifier
    pub id: String,
    /// Human-readable policy name
    pub name: String,
    /// Policy priority (higher = more important)
    pub priority: i32,
    /// Whether policy is enabled
    pub enabled: bool,
    /// Dimension filters (all must match for policy to apply)
    pub filters: PolicyFilters,
    /// Routing action when policy matches
    pub action: PolicyAction,
    /// Conditions for conditional application
    #[serde(default)]
    pub conditions: Vec<PolicyCondition>,
}

/// Dimension filters for policy matching
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct PolicyFilters {
    /// Required capabilities (model must have ALL)
    #[serde(default)]
    pub capabilities: Vec<CapabilityFilter>,

    /// Allowed tiers (model must be in ANY)
    #[serde(default)]
    pub tiers: Vec<TierCategory>,

    /// Allowed cost categories (model must be in ANY)
    #[serde(default)]
    pub costs: Vec<CostCategory>,

    /// Allowed context window categories (model must be in ANY)
    #[serde(default)]
    pub context_windows: Vec<ContextWindowCategory>,

    /// Allowed providers (model must be from ANY)
    #[serde(default)]
    pub providers: Vec<ProviderCategory>,

    /// Required modalities (model must support ALL)
    #[serde(default)]
    pub modalities: Vec<ModalityCategory>,
}

/// Capability filter with match mode
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CapabilityFilter {
    /// Capability to check
    pub capability: CapabilityCategory,
    /// Match mode: "require" (must have), "prefer" (bonus if has), "exclude" (must not have)
    #[serde(default = "default_capability_mode")]
    pub mode: String,
}

fn default_capability_mode() -> String {
    "require".to_string()
}

/// Input/output modality categories
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModalityCategory {
    /// Text input/output
    Text,
    /// Image input
    Image,
    /// Audio input/output
    Audio,
    /// Video input
    Video,
    /// Embedding output
    Embedding,
    /// Code generation
    Code,
}

impl ModalityCategory {
    pub fn as_str(&self) -> &str {
        match self {
            ModalityCategory::Text => "text",
            ModalityCategory::Image => "image",
            ModalityCategory::Audio => "audio",
            ModalityCategory::Video => "video",
            ModalityCategory::Embedding => "embedding",
            ModalityCategory::Code => "code",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "text" => Some(ModalityCategory::Text),
            "image" => Some(ModalityCategory::Image),
            "audio" => Some(ModalityCategory::Audio),
            "video" => Some(ModalityCategory::Video),
            "embedding" => Some(ModalityCategory::Embedding),
            "code" => Some(ModalityCategory::Code),
            _ => None,
        }
    }
}

/// Action to take when policy matches
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PolicyAction {
    /// Routing strategy: "prefer", "avoid", "block", "weight"
    #[serde(default = "default_action_type")]
    pub action_type: String,

    /// Weight adjustment (for "weight" action type)
    #[serde(default)]
    pub weight_factor: f64,

    /// Preferred providers in order
    #[serde(default)]
    pub preferred_providers: Vec<ProviderCategory>,

    /// Preferred model IDs
    #[serde(default)]
    pub preferred_models: Vec<String>,

    /// Models/providers to avoid
    #[serde(default)]
    pub avoid: Vec<String>,

    /// Maximum cost per million tokens (soft limit)
    #[serde(default)]
    pub max_cost_per_million: Option<f64>,

    /// Minimum context window required
    #[serde(default)]
    pub min_context_window: Option<usize>,
}

fn default_action_type() -> String {
    "prefer".to_string()
}

/// Conditional policy application
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PolicyCondition {
    /// Condition type
    pub condition_type: PolicyConditionType,

    /// Condition value
    pub value: String,

    /// Comparison operator
    #[serde(default = "default_operator")]
    pub operator: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PolicyConditionType {
    /// Time-based condition (hour of day)
    TimeOfDay,
    /// Day of week (0=Sunday)
    DayOfWeek,
    /// Request token count
    TokenCount,
    /// User/tenant ID
    TenantId,
    /// Model family
    ModelFamily,
    /// Custom metadata field
    Custom,
}

fn default_operator() -> String {
    "eq".to_string()
}

/// Policy evaluation context
#[derive(Debug, Clone, Default)]
pub struct PolicyContext {
    /// Current hour (0-23)
    pub hour_of_day: Option<i32>,
    /// Current day of week (0-6)
    pub day_of_week: Option<i32>,
    /// Estimated token count
    pub token_count: Option<usize>,
    /// Tenant identifier
    pub tenant_id: Option<String>,
    /// Model family being requested
    pub model_family: Option<String>,
    /// Custom metadata
    pub metadata: HashMap<String, String>,
}

/// Policy evaluation result
#[derive(Debug, Clone)]
pub struct PolicyMatch {
    /// Matched policy
    pub policy: RoutingPolicy,
    /// Match score (higher = better match)
    pub score: f64,
    /// Whether all conditions were met
    pub conditions_met: bool,
}

impl RoutingPolicy {
    /// Create a new policy with default settings
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

    /// Set policy priority
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// Add capability filter
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

    /// Add tier filter
    pub fn with_tier(mut self, tier: TierCategory) -> Self {
        if !self.filters.tiers.contains(&tier) {
            self.filters.tiers.push(tier);
        }
        self
    }

    /// Add provider filter
    pub fn with_provider(mut self, provider: ProviderCategory) -> Self {
        if !self.filters.providers.contains(&provider) {
            self.filters.providers.push(provider);
        }
        self
    }

    /// Set action type
    pub fn with_action(mut self, action_type: impl Into<String>) -> Self {
        self.action.action_type = action_type.into();
        self
    }

    /// Set weight factor
    pub fn with_weight_factor(mut self, factor: f64) -> Self {
        self.action.weight_factor = factor;
        self
    }

    /// Evaluate if this policy matches given context
    pub fn matches(&self, context: &PolicyContext) -> bool {
        if !self.enabled {
            return false;
        }

        // Check all conditions
        for condition in &self.conditions {
            if !self.evaluate_condition(condition, context) {
                return false;
            }
        }

        true
    }

    fn evaluate_condition(&self, condition: &PolicyCondition, context: &PolicyContext) -> bool {
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
                    context.metadata.get(parts[0]).map(|v| v.to_string())
                } else {
                    None
                }
            },
        };

        match actual_value {
            Some(actual) => {
                // Use numeric comparison for TokenCount to avoid lexicographic issues
                // e.g., "999" > "1000" lexicographically which is wrong for numbers
                if condition.condition_type == PolicyConditionType::TokenCount {
                    let actual_num = actual.parse::<i64>().unwrap_or(0);
                    let condition_num = condition.value.parse::<i64>().unwrap_or(0);
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
                        "in" => condition.value.split(',').any(|v| v.trim() == actual),
                        _ => false,
                    }
                }
            },
            None => false,
        }
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
