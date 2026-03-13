use super::types::*;

/// Policy registry for managing multiple routing policies
#[derive(Debug, Clone, Default)]
pub struct PolicyRegistry {
    policies: Vec<RoutingPolicy>,
}

impl PolicyRegistry {
    /// Create empty registry
    pub fn new() -> Self {
        Self {
            policies: Vec::new(),
        }
    }

    /// Add a policy
    pub fn add(&mut self, policy: RoutingPolicy) {
        self.policies.push(policy);
        self.sort_by_priority();
    }

    /// Remove a policy by ID
    pub fn remove(&mut self, id: &str) -> bool {
        let initial_len = self.policies.len();
        self.policies.retain(|p| p.id != id);
        self.policies.len() != initial_len
    }

    /// Get policy by ID
    pub fn get(&self, id: &str) -> Option<&RoutingPolicy> {
        self.policies.iter().find(|p| p.id == id)
    }

    /// Get all policies
    pub fn all(&self) -> &[RoutingPolicy] {
        &self.policies
    }

    /// Find matching policies for context
    pub fn find_matches(&self, context: &PolicyContext) -> Vec<&RoutingPolicy> {
        self.policies
            .iter()
            .filter(|p| p.matches(context))
            .collect()
    }

    /// Sort policies by priority (highest first)
    fn sort_by_priority(&mut self) {
        self.policies.sort_by(|a, b| b.priority.cmp(&a.priority));
    }

    /// Load policies from JSON
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        let policies: Vec<RoutingPolicy> = serde_json::from_str(json)?;
        let mut registry = Self { policies };
        registry.sort_by_priority();
        Ok(registry)
    }

    /// Export policies to JSON
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&self.policies)
    }
}
