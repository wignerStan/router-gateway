use super::types::{PolicyContext, PolicyLoadError, RoutingPolicy};

/// Registry for managing and querying routing policies.
#[derive(Debug, Clone, Default)]
pub struct PolicyRegistry {
    policies: Vec<RoutingPolicy>,
}

impl PolicyRegistry {
    /// Creates an empty registry.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            policies: Vec::new(),
        }
    }

    /// Adds a policy to the registry.
    ///
    /// Policies are sorted by priority (highest first) after insertion.
    ///
    /// # Examples
    ///
    /// ```
    /// use crate::registry::{PolicyRegistry, RoutingPolicy};
    ///
    /// let mut registry = PolicyRegistry::new();
    /// let policy = RoutingPolicy::new("pol-1", "Prefer Flagship");
    /// registry.add(policy);
    ///
    /// assert_eq!(registry.all().len(), 1);
    /// assert!(registry.get("pol-1").is_some());
    /// ```
    pub fn add(&mut self, policy: RoutingPolicy) {
        if self.policies.iter().any(|p| p.id == policy.id) {
            tracing::warn!("Replacing existing policy with duplicate ID: {}", policy.id);
            self.policies.retain(|p| p.id != policy.id);
        }
        self.policies.push(policy);
        self.sort_by_priority();
    }

    /// Removes a policy by ID, returning `true` if it existed.
    #[must_use]
    pub fn remove(&mut self, id: &str) -> bool {
        let initial_len = self.policies.len();
        self.policies.retain(|p| p.id != id);
        self.policies.len() != initial_len
    }

    /// Retrieves a policy by ID.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&RoutingPolicy> {
        self.policies.iter().find(|p| p.id == id)
    }

    /// Returns all policies sorted by priority.
    #[must_use]
    pub fn all(&self) -> &[RoutingPolicy] {
        &self.policies
    }

    /// Finds all policies whose conditions match the given context.
    #[must_use]
    pub fn find_matches(&self, context: &PolicyContext) -> Vec<&RoutingPolicy> {
        self.policies
            .iter()
            .filter(|p| p.matches(context))
            .collect()
    }

    /// Sorts policies by priority (highest first).
    fn sort_by_priority(&mut self) {
        self.policies.sort_by(|a, b| b.priority.cmp(&a.priority));
    }

    /// Deserializes policies from a JSON array string.
    ///
    /// # Errors
    ///
    /// Returns a serde JSON error on invalid input.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        let policies: Vec<RoutingPolicy> = serde_json::from_str(json)?;
        let mut registry = Self { policies };
        registry.sort_by_priority();
        Ok(registry)
    }

    /// Loads policies from a JSON file with schema validation.
    ///
    /// Expects the file format `{"policies": [...]}`.
    /// Validates against the embedded JSON schema before parsing.
    ///
    /// # Errors
    ///
    /// Returns [`PolicyLoadError::Io`] on read failure,
    /// [`PolicyLoadError::Schema`] on validation failure,
    /// or [`PolicyLoadError::Parse`] on deserialization failure.
    pub fn from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self, PolicyLoadError> {
        #[derive(serde::Deserialize)]
        struct PoliciesFile {
            policies: Vec<RoutingPolicy>,
        }

        let content = std::fs::read_to_string(path.as_ref())
            .map_err(|e| PolicyLoadError::Io(e.to_string()))?;

        let schema = Self::load_schema();
        let value = Self::validate_against_schema(&content, &schema)?;

        let file: PoliciesFile =
            serde_json::from_value(value).map_err(|e| PolicyLoadError::Parse(e.to_string()))?;

        let mut registry = Self {
            policies: file.policies,
        };
        registry.sort_by_priority();
        Ok(registry)
    }

    /// Loads the embedded JSON schema for policy validation.
    ///
    /// # Panics
    ///
    /// Panics if the embedded `policies.schema.json` is not valid JSON (a build-time bug).
    #[must_use]
    pub fn load_schema() -> serde_json::Value {
        // ALLOW: File is embedded at compile time via `include_str!` — if invalid, it is a build-time bug.
        #[allow(clippy::expect_used)]
        serde_json::from_str(include_str!("../../../config/policies.schema.json"))
            .expect("embedded policies.schema.json should be valid JSON")
    }

    /// Validates a JSON string against the policy schema.
    ///
    /// Returns the parsed `serde_json::Value` if valid.
    ///
    /// # Errors
    ///
    /// Returns [`PolicyLoadError::Parse`] if the JSON is malformed,
    /// or [`PolicyLoadError::Schema`] if validation fails.
    pub fn validate_against_schema(
        json: &str,
        schema: &serde_json::Value,
    ) -> Result<serde_json::Value, PolicyLoadError> {
        let instance: serde_json::Value =
            serde_json::from_str(json).map_err(|e| PolicyLoadError::Parse(e.to_string()))?;

        let validator = jsonschema::validator_for(schema)
            .map_err(|e| PolicyLoadError::Schema(e.to_string()))?;

        if validator.is_valid(&instance) {
            return Ok(instance);
        }

        let validation = match validator.validate(&instance) {
            Ok(()) => {
                return Err(PolicyLoadError::Schema(
                    "validation failed but no errors produced".to_string(),
                ));
            },
            Err(errs) => errs,
        };
        let mut errors: Vec<String> = validation.map(|err| format!("  - {err}")).collect();
        if errors.is_empty() {
            return Err(PolicyLoadError::Schema(
                "validation failed but no errors produced".to_string(),
            ));
        }
        errors.sort();
        Err(PolicyLoadError::Schema(format!(
            "Schema validation failed:\n{}",
            errors.join("\n")
        )))
    }

    /// Serializes all policies to a pretty-printed JSON string.
    ///
    /// # Errors
    ///
    /// Returns a serde JSON error on serialization failure.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&self.policies)
    }
}
