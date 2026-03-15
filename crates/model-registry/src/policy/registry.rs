use super::types::{PolicyContext, PolicyLoadError, RoutingPolicy};

/// Policy registry for managing multiple routing policies
#[derive(Debug, Clone, Default)]
pub struct PolicyRegistry {
    policies: Vec<RoutingPolicy>,
}

impl PolicyRegistry {
    /// Create empty registry
    pub const fn new() -> Self {
        Self {
            policies: Vec::new(),
        }
    }

    /// Add a policy to the registry.
    ///
    /// Policies are sorted by priority (highest first) after insertion.
    ///
    /// # Examples
    ///
    /// ```
    /// use model_registry::{PolicyRegistry, RoutingPolicy};
    ///
    /// let mut registry = PolicyRegistry::new();
    /// let policy = RoutingPolicy::new("pol-1", "Prefer Flagship");
    /// registry.add(policy);
    ///
    /// assert_eq!(registry.all().len(), 1);
    /// assert!(registry.get("pol-1").is_some());
    /// ```
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

    /// Load policies from a JSON file with schema validation.
    ///
    /// Expects the file format `{"policies": [...]}`.
    /// Validates against the embedded JSON schema before parsing.
    pub fn from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self, PolicyLoadError> {
        let content = std::fs::read_to_string(path.as_ref())
            .map_err(|e| PolicyLoadError::Io(e.to_string()))?;

        let schema = Self::load_schema();
        let value = Self::validate_against_schema(&content, &schema)?;

        #[derive(serde::Deserialize)]
        struct PoliciesFile {
            policies: Vec<RoutingPolicy>,
        }

        let file: PoliciesFile =
            serde_json::from_value(value).map_err(|e| PolicyLoadError::Parse(e.to_string()))?;

        let mut registry = Self {
            policies: file.policies,
        };
        registry.sort_by_priority();
        Ok(registry)
    }

    /// Load the embedded JSON schema for policy validation.
    pub fn load_schema() -> serde_json::Value {
        serde_json::from_str(include_str!("../../../../config/policies.schema.json"))
            .expect("embedded policies.schema.json should be valid JSON")
    }

    /// Validate a JSON string against the policy schema.
    ///
    /// Returns the parsed `serde_json::Value` if valid, `Err` with a description of all violations.
    pub fn validate_against_schema(
        json: &str,
        schema: &serde_json::Value,
    ) -> Result<serde_json::Value, PolicyLoadError> {
        let instance: serde_json::Value =
            serde_json::from_str(json).map_err(|e| PolicyLoadError::Parse(e.to_string()))?;

        let validator = jsonschema::validator_for(schema)
            .map_err(|e| PolicyLoadError::Schema(e.to_string()))?;

        if validator.is_valid(&instance) {
            Ok(instance)
        } else {
            let mut errors: Vec<String> = validator
                .validate(&instance)
                .expect_err("validation should fail since is_valid returned false")
                .map(|err| format!("  - {err}"))
                .collect();
            errors.sort();
            Err(PolicyLoadError::Schema(format!(
                "Schema validation failed:\n{}",
                errors.join("\n")
            )))
        }
    }

    /// Export policies to JSON
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&self.policies)
    }
}
