//! Hard constraint filtering for route candidates
//!
//! This module filters route candidates through hard constraints including
//! capability mismatches, context overflow, disabled providers, and tenant policies.

use crate::candidate::{check_capability_support, RouteCandidate, TokenFitStatus};
use crate::classification::ClassifiedRequest;
use model_registry::{PolicyContext, PolicyMatcher};
use std::fmt;

/// Filter result with reason for rejection
#[derive(Debug, Clone, PartialEq)]
pub enum FilterResult {
    /// Candidate passed all filters
    Accepted,
    /// Candidate was rejected
    Rejected { reason: String },
}

impl fmt::Display for FilterResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FilterResult::Accepted => write!(f, "accepted"),
            FilterResult::Rejected { reason } => write!(f, "rejected: {}", reason),
        }
    }
}

impl FilterResult {
    /// Check if candidate was accepted
    pub fn is_accepted(&self) -> bool {
        matches!(self, FilterResult::Accepted)
    }
}

/// Hard constraint filter for route candidates
#[derive(Clone)]
pub struct ConstraintFilter {
    /// Disabled providers (empty = no restrictions)
    disabled_providers: Vec<String>,
    /// Policy matcher for policy-aware filtering
    policy_matcher: Option<PolicyMatcher>,
    /// Tenant ID for policy context
    tenant_id: Option<String>,
}

impl ConstraintFilter {
    /// Create a new constraint filter
    pub fn new() -> Self {
        Self {
            disabled_providers: Vec::new(),
            policy_matcher: None,
            tenant_id: None,
        }
    }

    /// Add a disabled provider
    pub fn add_disabled_provider(&mut self, provider: String) -> &mut Self {
        self.disabled_providers.push(provider);
        self
    }

    /// Set policy matcher for policy-aware filtering
    pub fn set_policy_matcher(&mut self, matcher: PolicyMatcher) -> &mut Self {
        self.policy_matcher = Some(matcher);
        self
    }

    /// Set tenant ID for policy context
    pub fn set_tenant_id(&mut self, tenant_id: String) -> &mut Self {
        self.tenant_id = Some(tenant_id);
        self
    }

    /// Filter candidates through hard constraints
    ///
    /// # Scenarios
    /// - Capability mismatch: Reject
    /// - Context overflow: Reject
    /// - Provider disabled: Reject
    /// - Policy violation: Reject
    pub fn filter(
        &self,
        candidates: Vec<RouteCandidate>,
        request: &ClassifiedRequest,
    ) -> Vec<RouteCandidate> {
        candidates
            .into_iter()
            .filter(|candidate| {
                let result = self.check_constraints(candidate, request);
                result.is_accepted()
            })
            .collect()
    }

    /// Check constraints for a single candidate
    pub fn check_constraints(
        &self,
        candidate: &RouteCandidate,
        request: &ClassifiedRequest,
    ) -> FilterResult {
        // 1. Check capability mismatch
        let capability_support =
            check_capability_support(&request.required_capabilities, &candidate.model_info);
        if !capability_support.is_supported() {
            if let Some(desc) = capability_support.missing_description() {
                return FilterResult::Rejected {
                    reason: format!("capability mismatch: missing {}", desc),
                };
            }
        }

        // 2. Check context overflow (only if we have token estimate)
        if candidate.token_fit == TokenFitStatus::Exceeds {
            return FilterResult::Rejected {
                reason: format!(
                    "context overflow: {} tokens exceeds model {} context window of {}",
                    request.estimated_tokens,
                    candidate.model_id,
                    candidate.model_info.context_window
                ),
            };
        }

        // 3. Check if provider is disabled
        if self.disabled_providers.contains(&candidate.provider) {
            return FilterResult::Rejected {
                reason: format!("provider disabled: {}", candidate.provider),
            };
        }

        // 4. Check policy violations (if policy matcher is configured)
        if let Some(matcher) = &self.policy_matcher {
            if let Some(reason) = self.check_policy_violation(matcher, candidate, request) {
                return FilterResult::Rejected { reason };
            }
        }

        FilterResult::Accepted
    }

    /// Check policy violations for a candidate
    fn check_policy_violation(
        &self,
        matcher: &PolicyMatcher,
        candidate: &RouteCandidate,
        request: &ClassifiedRequest,
    ) -> Option<String> {
        // Build policy context
        let context = PolicyContext {
            tenant_id: self.tenant_id.clone(),
            token_count: Some(request.estimated_tokens as usize),
            hour_of_day: None,
            day_of_week: None,
            model_family: None,
            metadata: Default::default(),
        };

        // Check if any policy blocks this candidate
        let is_blocked = matcher.is_blocked(&candidate.model_info, &context);
        if is_blocked {
            return Some("policy violation: blocked by policy".to_string());
        }

        None
    }
}

impl Default for ConstraintFilter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::candidate::TokenFitStatus;
    use crate::classification::{QualityPreference, RequestFormat, RequiredCapabilities};
    use model_registry::{DataSource, ModelCapabilities, ModelInfo, RateLimits};

    fn create_test_model(
        id: &str,
        provider: &str,
        context_window: usize,
        vision: bool,
    ) -> ModelInfo {
        ModelInfo {
            id: id.to_string(),
            name: format!("Test Model {}", id),
            provider: provider.to_string(),
            context_window,
            max_output_tokens: 4096,
            input_price_per_million: 1.0,
            output_price_per_million: 2.0,
            capabilities: ModelCapabilities {
                streaming: true,
                tools: true,
                vision,
                thinking: false,
            },
            rate_limits: RateLimits {
                requests_per_minute: 60,
                tokens_per_minute: 90000,
            },
            source: DataSource::Static,
        }
    }

    fn create_test_request(
        estimated_tokens: u32,
        required_capabilities: RequiredCapabilities,
    ) -> ClassifiedRequest {
        ClassifiedRequest {
            required_capabilities,
            estimated_tokens,
            format: RequestFormat::OpenAI,
            quality_preference: QualityPreference::Balanced,
        }
    }

    #[test]
    fn test_filter_empty_list() {
        let filter = ConstraintFilter::new();
        let request = create_test_request(1000, RequiredCapabilities::default());

        let result = filter.filter(Vec::new(), &request);
        assert!(result.is_empty());
    }

    #[test]
    fn test_filter_capability_mismatch() {
        let filter = ConstraintFilter::new();

        // Create a model without vision
        let model = create_test_model("test", "test", 200000, false);

        let candidate = RouteCandidate {
            credential_id: "cred-1".to_string(),
            model_id: "test".to_string(),
            provider: "test".to_string(),
            model_info: model,
            estimated_cost: 0.0,
            token_fit: TokenFitStatus::Fits,
        };

        // Request requires vision
        let request = create_test_request(
            1000,
            RequiredCapabilities {
                vision: true,
                tools: false,
                streaming: false,
                thinking: false,
            },
        );

        let result = filter.check_constraints(&candidate, &request);
        assert!(!result.is_accepted());
        let reason = match &result {
            FilterResult::Rejected { reason } => reason.as_str(),
            _ => "",
        };
        assert!(reason.contains("capability mismatch"));
    }

    #[test]
    fn test_filter_context_overflow() {
        let filter = ConstraintFilter::new();

        let model = create_test_model("test", "test", 100000, true);

        let candidate = RouteCandidate {
            credential_id: "cred-1".to_string(),
            model_id: "test".to_string(),
            provider: "test".to_string(),
            model_info: model,
            estimated_cost: 0.0,
            token_fit: TokenFitStatus::Exceeds,
        };

        let request = create_test_request(200000, RequiredCapabilities::default());

        let result = filter.check_constraints(&candidate, &request);
        assert!(!result.is_accepted());
        let reason = match &result {
            FilterResult::Rejected { reason } => reason.as_str(),
            _ => "",
        };
        assert!(reason.contains("context overflow"));
    }

    #[test]
    fn test_filter_provider_disabled() {
        let mut filter = ConstraintFilter::new();
        filter.add_disabled_provider("blocked-provider".to_string());

        let model = create_test_model("test", "blocked-provider", 200000, true);

        let candidate = RouteCandidate {
            credential_id: "cred-1".to_string(),
            model_id: "test".to_string(),
            provider: "blocked-provider".to_string(),
            model_info: model,
            estimated_cost: 0.0,
            token_fit: TokenFitStatus::Fits,
        };

        let request = create_test_request(1000, RequiredCapabilities::default());

        let result = filter.check_constraints(&candidate, &request);
        assert!(!result.is_accepted());
        let reason = match &result {
            FilterResult::Rejected { reason } => reason.as_str(),
            _ => "",
        };
        assert!(reason.contains("provider disabled"));
    }

    #[test]
    fn test_filter_provider_enabled() {
        let mut filter = ConstraintFilter::new();
        filter.add_disabled_provider("blocked-provider".to_string());

        let model = create_test_model("test", "allowed-provider", 200000, true);

        let candidate = RouteCandidate {
            credential_id: "cred-1".to_string(),
            model_id: "test".to_string(),
            provider: "allowed-provider".to_string(),
            model_info: model,
            estimated_cost: 0.0,
            token_fit: TokenFitStatus::Fits,
        };

        let request = create_test_request(1000, RequiredCapabilities::default());

        let result = filter.check_constraints(&candidate, &request);
        assert!(result.is_accepted());
    }

    #[test]
    fn test_filter_accepted_candidate() {
        let filter = ConstraintFilter::new();

        let model = create_test_model("test", "test", 200000, true);

        let candidate = RouteCandidate {
            credential_id: "cred-1".to_string(),
            model_id: "test".to_string(),
            provider: "test".to_string(),
            model_info: model,
            estimated_cost: 0.0,
            token_fit: TokenFitStatus::Fits,
        };

        let request = create_test_request(1000, RequiredCapabilities::default());

        let result = filter.check_constraints(&candidate, &request);
        assert!(result.is_accepted());
    }

    #[test]
    fn test_filter_multiple_candidates() {
        let mut filter = ConstraintFilter::new();
        filter.add_disabled_provider("blocked-provider".to_string());

        let model1 = create_test_model("model1", "allowed-provider", 200000, true);
        let model2 = create_test_model("model2", "blocked-provider", 200000, true);

        let candidates = vec![
            RouteCandidate {
                credential_id: "cred-1".to_string(),
                model_id: "model1".to_string(),
                provider: "allowed-provider".to_string(),
                model_info: model1,
                estimated_cost: 0.0,
                token_fit: TokenFitStatus::Fits,
            },
            RouteCandidate {
                credential_id: "cred-2".to_string(),
                model_id: "model2".to_string(),
                provider: "blocked-provider".to_string(),
                model_info: model2,
                estimated_cost: 0.0,
                token_fit: TokenFitStatus::Fits,
            },
        ];

        let request = create_test_request(1000, RequiredCapabilities::default());

        let result = filter.filter(candidates, &request);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].provider, "allowed-provider");
    }

    #[test]
    fn test_filter_token_fit_unknown() {
        let filter = ConstraintFilter::new();

        let model = create_test_model("test", "test", 200000, true);

        let candidate = RouteCandidate {
            credential_id: "cred-1".to_string(),
            model_id: "test".to_string(),
            provider: "test".to_string(),
            model_info: model,
            estimated_cost: 0.0,
            token_fit: TokenFitStatus::Unknown,
        };

        // Request with unknown tokens (0) should not be rejected
        let request = create_test_request(0, RequiredCapabilities::default());

        let result = filter.check_constraints(&candidate, &request);
        assert!(result.is_accepted());
    }

    #[test]
    fn test_filter_context_exact_fit() {
        let filter = ConstraintFilter::new();

        let model = create_test_model("test", "test", 100000, true);

        let candidate = RouteCandidate {
            credential_id: "cred-1".to_string(),
            model_id: "test".to_string(),
            provider: "test".to_string(),
            model_info: model,
            estimated_cost: 0.0,
            token_fit: TokenFitStatus::Fits, // Fits within context
        };

        let request = create_test_request(100000, RequiredCapabilities::default());

        let result = filter.check_constraints(&candidate, &request);
        assert!(result.is_accepted());
    }

    #[test]
    fn test_filter_result_is_accepted() {
        assert!(FilterResult::Accepted.is_accepted());
        assert!(!FilterResult::Rejected {
            reason: "test".to_string()
        }
        .is_accepted());
    }

    #[test]
    fn test_constraint_filter_default() {
        let filter = ConstraintFilter::default();
        assert!(filter.disabled_providers.is_empty());
        assert!(filter.policy_matcher.is_none());
        assert!(filter.tenant_id.is_none());
    }

    #[test]
    fn test_constraint_filter_chainable() {
        let mut filter = ConstraintFilter::new();
        filter
            .add_disabled_provider("provider1".to_string())
            .add_disabled_provider("provider2".to_string())
            .set_tenant_id("tenant-1".to_string());

        assert_eq!(filter.disabled_providers.len(), 2);
        assert_eq!(filter.tenant_id, Some("tenant-1".to_string()));
    }

    #[test]
    fn test_filter_capabilities_none_required() {
        let filter = ConstraintFilter::new();

        // Model with minimal capabilities
        let mut model = create_test_model("test", "test", 200000, false);
        model.capabilities.tools = false;
        model.capabilities.streaming = false;

        let candidate = RouteCandidate {
            credential_id: "cred-1".to_string(),
            model_id: "test".to_string(),
            provider: "test".to_string(),
            model_info: model,
            estimated_cost: 0.0,
            token_fit: TokenFitStatus::Fits,
        };

        // No capabilities required
        let request = create_test_request(1000, RequiredCapabilities::default());

        let result = filter.check_constraints(&candidate, &request);
        assert!(result.is_accepted());
    }

    // ============================================================
    // Edge Case Tests for ConstraintFilter
    // ============================================================

    #[test]
    fn test_filter_all_providers_disabled_returns_empty() {
        let mut filter = ConstraintFilter::new();
        filter.add_disabled_provider("provider1".to_string());
        filter.add_disabled_provider("provider2".to_string());

        let model1 = create_test_model("model1", "provider1", 200000, true);
        let model2 = create_test_model("model2", "provider2", 200000, true);

        let candidates = vec![
            RouteCandidate {
                credential_id: "cred-1".to_string(),
                model_id: "model1".to_string(),
                provider: "provider1".to_string(),
                model_info: model1,
                estimated_cost: 0.0,
                token_fit: TokenFitStatus::Fits,
            },
            RouteCandidate {
                credential_id: "cred-2".to_string(),
                model_id: "model2".to_string(),
                provider: "provider2".to_string(),
                model_info: model2,
                estimated_cost: 0.0,
                token_fit: TokenFitStatus::Fits,
            },
        ];

        let request = create_test_request(1000, RequiredCapabilities::default());

        let result = filter.filter(candidates, &request);
        assert!(
            result.is_empty(),
            "All providers disabled should return empty list"
        );
    }

    #[test]
    fn test_filter_empty_disabled_providers_accepts_all() {
        let filter = ConstraintFilter::new();

        let model1 = create_test_model("model1", "provider1", 200000, true);
        let model2 = create_test_model("model2", "provider2", 200000, true);

        let candidates = vec![
            RouteCandidate {
                credential_id: "cred-1".to_string(),
                model_id: "model1".to_string(),
                provider: "provider1".to_string(),
                model_info: model1,
                estimated_cost: 0.0,
                token_fit: TokenFitStatus::Fits,
            },
            RouteCandidate {
                credential_id: "cred-2".to_string(),
                model_id: "model2".to_string(),
                provider: "provider2".to_string(),
                model_info: model2,
                estimated_cost: 0.0,
                token_fit: TokenFitStatus::Fits,
            },
        ];

        let request = create_test_request(1000, RequiredCapabilities::default());

        let result = filter.filter(candidates, &request);
        assert_eq!(
            result.len(),
            2,
            "Empty disabled providers should accept all candidates"
        );
    }

    #[test]
    fn test_filter_multiple_capabilities_required_in_combination() {
        let filter = ConstraintFilter::new();

        // Model with only vision
        let mut model_vision_only = create_test_model("vision-only", "test", 200000, true);
        model_vision_only.capabilities.tools = false;

        // Model with vision and tools
        let model_vision_tools = create_test_model("vision-tools", "test", 200000, true);
        // tools already true by default

        // Request requires BOTH vision AND tools
        let request = create_test_request(
            1000,
            RequiredCapabilities {
                vision: true,
                tools: true,
                streaming: false,
                thinking: false,
            },
        );

        let candidate_vision_only = RouteCandidate {
            credential_id: "cred-1".to_string(),
            model_id: "vision-only".to_string(),
            provider: "test".to_string(),
            model_info: model_vision_only,
            estimated_cost: 0.0,
            token_fit: TokenFitStatus::Fits,
        };

        let candidate_vision_tools = RouteCandidate {
            credential_id: "cred-2".to_string(),
            model_id: "vision-tools".to_string(),
            provider: "test".to_string(),
            model_info: model_vision_tools,
            estimated_cost: 0.0,
            token_fit: TokenFitStatus::Fits,
        };

        // Vision-only should be rejected
        let result = filter.check_constraints(&candidate_vision_only, &request);
        assert!(
            !result.is_accepted(),
            "Vision-only model should be rejected when tools required"
        );

        // Vision+tools should be accepted
        let result = filter.check_constraints(&candidate_vision_tools, &request);
        assert!(
            result.is_accepted(),
            "Model with vision and tools should be accepted"
        );
    }

    #[test]
    fn test_filter_context_at_exact_boundary_fits() {
        let filter = ConstraintFilter::new();

        // Model with exactly 100000 context window
        let model = create_test_model("test", "test", 100000, true);

        // Request with exactly 100000 tokens should fit (not exceed)
        let candidate = RouteCandidate {
            credential_id: "cred-1".to_string(),
            model_id: "test".to_string(),
            provider: "test".to_string(),
            model_info: model,
            estimated_cost: 0.0,
            token_fit: TokenFitStatus::Fits, // At boundary still fits
        };

        let request = create_test_request(100000, RequiredCapabilities::default());

        let result = filter.check_constraints(&candidate, &request);
        assert!(
            result.is_accepted(),
            "Token count at exact boundary should fit"
        );
    }

    #[test]
    fn test_filter_context_one_over_boundary_exceeds() {
        let filter = ConstraintFilter::new();

        let model = create_test_model("test", "test", 100000, true);

        let candidate = RouteCandidate {
            credential_id: "cred-1".to_string(),
            model_id: "test".to_string(),
            provider: "test".to_string(),
            model_info: model,
            estimated_cost: 0.0,
            token_fit: TokenFitStatus::Exceeds, // Exceeds by 1
        };

        let request = create_test_request(100001, RequiredCapabilities::default());

        let result = filter.check_constraints(&candidate, &request);
        assert!(
            !result.is_accepted(),
            "Token count exceeding boundary should be rejected"
        );
    }

    #[test]
    fn test_filter_all_capabilities_required() {
        let filter = ConstraintFilter::new();

        let mut model_full = create_test_model("full", "test", 200000, true);
        model_full.capabilities.thinking = true;

        let mut model_partial = create_test_model("partial", "test", 200000, true);
        model_partial.capabilities.thinking = false;

        // Request requires ALL capabilities
        let request = create_test_request(
            1000,
            RequiredCapabilities {
                vision: true,
                tools: true,
                streaming: true,
                thinking: true,
            },
        );

        let candidate_full = RouteCandidate {
            credential_id: "cred-1".to_string(),
            model_id: "full".to_string(),
            provider: "test".to_string(),
            model_info: model_full,
            estimated_cost: 0.0,
            token_fit: TokenFitStatus::Fits,
        };

        let candidate_partial = RouteCandidate {
            credential_id: "cred-2".to_string(),
            model_id: "partial".to_string(),
            provider: "test".to_string(),
            model_info: model_partial,
            estimated_cost: 0.0,
            token_fit: TokenFitStatus::Fits,
        };

        assert!(
            filter
                .check_constraints(&candidate_full, &request)
                .is_accepted(),
            "Model with all capabilities should be accepted"
        );
        assert!(
            !filter
                .check_constraints(&candidate_partial, &request)
                .is_accepted(),
            "Model missing thinking capability should be rejected"
        );
    }
}
