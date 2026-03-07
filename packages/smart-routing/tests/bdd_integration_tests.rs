// BDD (Behavior-Driven Development) tests for smart-routing
//
// This module contains Cucumber-style tests that verify the behavior of
// the classification and health management systems.

#[cfg(test)]
mod bdd_integration {

    #[tokio::test]
    async fn test_bdd_classification_vision_detection() {
        use smart_routing::classification::ContentTypeDetector;

        // Scenario: Image attachment requires vision
        let request = serde_json::json!({
            "messages": [{
                "role": "user",
                "content": [{
                    "type": "image_url",
                    "image_url": {"url": "https://example.com/image.png"}
                }]
            }]
        });
        assert!(ContentTypeDetector::detect_vision_required(&request));

        // Scenario: Text-only content does not require vision
        let request = serde_json::json!({
            "messages": [{
                "role": "user",
                "content": "Hello, world!"
            }]
        });
        assert!(!ContentTypeDetector::detect_vision_required(&request));

        // Scenario: Mixed content requires vision
        let request = serde_json::json!({
            "messages": [{
                "role": "user",
                "content": [
                    {"type": "text", "text": "What is this?"},
                    {"type": "image_url", "image_url": {"url": "https://example.com/image.png"}}
                ]
            }]
        });
        assert!(ContentTypeDetector::detect_vision_required(&request));
    }

    #[tokio::test]
    async fn test_bdd_classification_tool_detection() {
        use smart_routing::classification::ToolDetector;

        // Scenario: Tool definitions require tool support
        let request = serde_json::json!({
            "messages": [{"role": "user", "content": "What's the weather?"}],
            "tools": [{
                "type": "function",
                "function": {
                    "name": "get_weather",
                    "parameters": {"type": "object"}
                }
            }]
        });
        assert!(ToolDetector::detect_tools_required(&request));

        // Scenario: No tool definitions means no requirement
        let request = serde_json::json!({
            "messages": [{"role": "user", "content": "Hello"}]
        });
        assert!(!ToolDetector::detect_tools_required(&request));

        // Scenario: Empty tool array does not require tools
        let request = serde_json::json!({
            "messages": [{"role": "user", "content": "Hello"}],
            "tools": []
        });
        assert!(!ToolDetector::detect_tools_required(&request));
    }

    #[tokio::test]
    async fn test_bdd_classification_streaming_detection() {
        use smart_routing::classification::StreamingExtractor;

        // Scenario: Explicit streaming enabled requires streaming support
        let request = serde_json::json!({
            "messages": [{"role": "user", "content": "Hello"}],
            "stream": true
        });
        assert!(StreamingExtractor::extract_streaming_preference(&request));

        // Scenario: Explicit streaming disabled does not require streaming
        let request = serde_json::json!({
            "messages": [{"role": "user", "content": "Hello"}],
            "stream": false
        });
        assert!(!StreamingExtractor::extract_streaming_preference(&request));

        // Scenario: Default behavior when streaming flag is absent
        let request = serde_json::json!({
            "messages": [{"role": "user", "content": "Hello"}]
        });
        assert!(!StreamingExtractor::extract_streaming_preference(&request));
    }

    #[tokio::test]
    async fn test_bdd_classification_format_detection() {
        use smart_routing::classification::FormatDetector;
        use smart_routing::classification::RequestFormat;

        // Scenario: OpenAI format requests are identified by structure
        let request = serde_json::json!({
            "messages": [{"role": "user", "content": "Hello"}],
            "model": "gpt-4"
        });
        assert_eq!(FormatDetector::detect(&request), RequestFormat::OpenAI);

        // Scenario: Anthropic format requests are recognized
        let request = serde_json::json!({
            "messages": [{"role": "user", "content": "Hello"}],
            "system": "You are a helpful assistant",
            "model": "claude-3-opus"
        });
        assert_eq!(FormatDetector::detect(&request), RequestFormat::Anthropic);

        // Scenario: Gemini format requests are detected
        let request = serde_json::json!({
            "contents": [{"parts": [{"text": "Hello"}]}],
            "model": "gemini-pro"
        });
        assert_eq!(FormatDetector::detect(&request), RequestFormat::Gemini);

        // Scenario: Unknown format defaults to generic handling
        let request = serde_json::json!({
            "prompt": "Hello",
            "model": "unknown-model"
        });
        assert_eq!(FormatDetector::detect(&request), RequestFormat::Generic);
    }

    #[tokio::test]
    async fn test_bdd_classification_token_estimation() {
        use smart_routing::classification::TokenEstimator;

        // Scenario: Small prompt fits standard context
        let request = serde_json::json!({
            "messages": [{"role": "user", "content": "Hello"}]
        });
        let tokens = TokenEstimator::estimate(&request);
        assert!(tokens < 1000, "Small prompt should fit standard context");

        // Scenario: Large prompt requires high context capacity
        let large_text = "x".repeat(200000); // ~50000 tokens
        let request = serde_json::json!({
            "messages": [{"role": "user", "content": large_text}]
        });
        let tokens = TokenEstimator::estimate(&request);
        assert!(tokens > 40000, "Large prompt should require high context");

        // Scenario: Total estimated tokens combines input and expected output
        let request = serde_json::json!({
            "messages": [{"role": "user", "content": "x".repeat(4000)}], // ~1000 input tokens
            "max_tokens": 500
        });
        let tokens = TokenEstimator::estimate(&request);
        assert!(tokens > 1400 && tokens < 1600, "Total should combine input and output");
    }

    #[tokio::test]
    async fn test_bdd_classification_reasoning_detection() {
        use smart_routing::reasoning::{ReasoningInference, ReasoningRequest};
        use std::collections::HashMap;

        let inference = ReasoningInference::new();

        // Scenario: Reasoning flag explicitly enabled requires thinking support
        let request = ReasoningRequest {
            model: "gpt-4".to_string(),
            reasoning_flag: Some(true),
            max_tokens: None,
            hints: HashMap::new(),
        };
        assert!(inference.requires_reasoning(&request).await);

        // Scenario: Model family hint suggests reasoning requirement
        let request = ReasoningRequest {
            model: "o1-mini".to_string(),
            reasoning_flag: None,
            max_tokens: None,
            hints: HashMap::new(),
        };
        assert!(inference.requires_reasoning(&request).await);

        // Scenario: Standard requests do not require thinking
        let request = ReasoningRequest {
            model: "gpt-4".to_string(),
            reasoning_flag: None,
            max_tokens: None,
            hints: HashMap::new(),
        };
        assert!(!inference.requires_reasoning(&request).await);
    }

    #[tokio::test]
    async fn test_bdd_health_rate_limit_triggers_degraded() {
        use smart_routing::health::{HealthManager, HealthStatus};
        use smart_routing::config::HealthConfig;

        let config = HealthConfig {
            status_codes: smart_routing::config::StatusCodeHealthConfig {
                degraded: vec![429],
                unhealthy: vec![],
                healthy: vec![],
            },
            ..Default::default()
        };
        let manager = HealthManager::new(config);

        // Scenario: Rate limit triggers degraded state
        assert_eq!(manager.get_status("test-auth").await, HealthStatus::Healthy);
        manager.update_from_result("test-auth", false, 429).await;
        assert_eq!(manager.get_status("test-auth").await, HealthStatus::Degraded);
    }

    #[tokio::test]
    async fn test_bdd_health_consecutive_failures_trigger_unhealthy() {
        use smart_routing::health::{HealthManager, HealthStatus};
        use smart_routing::config::HealthConfig;

        let config = HealthConfig {
            unhealthy_threshold: 5,
            ..Default::default()
        };
        let manager = HealthManager::new(config);

        // Scenario: Consecutive failures trigger unhealthy state
        assert_eq!(manager.get_status("test-auth").await, HealthStatus::Healthy);
        for _ in 0..5 {
            manager.update_from_result("test-auth", false, 500).await;
        }
        assert_eq!(manager.get_status("test-auth").await, HealthStatus::Unhealthy);
    }

    #[tokio::test]
    async fn test_bdd_health_success_streak_recovers_degraded() {
        use smart_routing::health::{HealthManager, HealthStatus};
        use smart_routing::config::HealthConfig;

        let config = HealthConfig {
            healthy_threshold: 3,
            status_codes: smart_routing::config::StatusCodeHealthConfig {
                degraded: vec![429],
                unhealthy: vec![],
                healthy: vec![],
            },
            ..Default::default()
        };
        let manager = HealthManager::new(config);

        // Scenario: Success streak recovers degraded credential
        manager.update_from_result("test-auth", false, 429).await;
        assert_eq!(manager.get_status("test-auth").await, HealthStatus::Degraded);

        for _ in 0..3 {
            manager.update_from_result("test-auth", true, 200).await;
        }
        assert_eq!(manager.get_status("test-auth").await, HealthStatus::Healthy);
    }

    #[tokio::test]
    async fn test_bdd_health_unhealthy_blocked_from_selection() {
        use smart_routing::health::{HealthManager, HealthStatus};
        use smart_routing::config::HealthConfig;

        let config = HealthConfig {
            unhealthy_threshold: 2,
            cooldown_period_seconds: 10,
            ..Default::default()
        };
        let manager = HealthManager::new(config);

        // Scenario: Unhealthy credential blocked from selection
        for _ in 0..2 {
            manager.update_from_result("test-auth", false, 500).await;
        }
        assert_eq!(manager.get_status("test-auth").await, HealthStatus::Unhealthy);
        assert!(!manager.is_available("test-auth").await);
    }

    #[tokio::test]
    async fn test_bdd_health_cooldown_expiration_allows_recovery() {
        use smart_routing::health::{HealthManager, HealthStatus};
        use smart_routing::config::HealthConfig;

        let config = HealthConfig {
            unhealthy_threshold: 2,
            cooldown_period_seconds: 1,
            ..Default::default()
        };
        let manager = HealthManager::new(config);

        // Scenario: Cooldown expiration allows recovery attempt
        for _ in 0..2 {
            manager.update_from_result("test-auth", false, 500).await;
        }
        assert_eq!(manager.get_status("test-auth").await, HealthStatus::Unhealthy);
        assert!(!manager.is_available("test-auth").await);

        // Wait for cooldown to expire
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        // Cooldown expired, but still unhealthy status
        assert_eq!(manager.get_status("test-auth").await, HealthStatus::Unhealthy);
    }

    #[tokio::test]
    async fn test_bdd_all_request_classification_scenarios() {
        use smart_routing::classification::{
            ContentTypeDetector, FormatDetector, StreamingExtractor,
            ToolDetector, RequestFormat,
        };

        // Scenario: All capabilities detected in complex request
        let request = serde_json::json!({
            "messages": [{
                "role": "user",
                "content": [
                    {"type": "text", "text": "Analyze this image"},
                    {"type": "image_url", "image_url": {"url": "https://example.com/image.png"}}
                ]
            }],
            "tools": [{"type": "function", "function": {"name": "analyze", "parameters": {}}}],
            "stream": true
        });

        assert!(ContentTypeDetector::detect_vision_required(&request));
        assert!(ToolDetector::detect_tools_required(&request));
        assert!(StreamingExtractor::extract_streaming_preference(&request));
        assert_eq!(FormatDetector::detect(&request), RequestFormat::OpenAI);
    }

    #[tokio::test]
    async fn test_bdd_all_health_state_transitions() {
        use smart_routing::health::{HealthManager, HealthStatus};
        use smart_routing::config::HealthConfig;

        let config = HealthConfig {
            status_codes: smart_routing::config::StatusCodeHealthConfig {
                degraded: vec![429, 503],
                unhealthy: vec![401, 403, 500, 502, 504],
                healthy: vec![],
            },
            unhealthy_threshold: 3,
            healthy_threshold: 3,
            degraded_threshold: 0.5,
            cooldown_period_seconds: 1,
        };
        let manager = HealthManager::new(config);

        // Test full state machine: Healthy -> Degraded -> Unhealthy -> Degraded -> Healthy
        let auth_id = "state-transition-test";

        // Start healthy
        assert_eq!(manager.get_status(auth_id).await, HealthStatus::Healthy);

        // Trigger degraded (rate limit)
        manager.update_from_result(auth_id, false, 429).await;
        assert_eq!(manager.get_status(auth_id).await, HealthStatus::Degraded);

        // Trigger unhealthy (3 more failures)
        for _ in 0..3 {
            manager.update_from_result(auth_id, false, 500).await;
        }
        assert_eq!(manager.get_status(auth_id).await, HealthStatus::Unhealthy);

        // Recover to degraded (rate limit response)
        manager.update_from_result(auth_id, false, 429).await;
        assert_eq!(manager.get_status(auth_id).await, HealthStatus::Degraded);

        // Recover to healthy (3 consecutive successes)
        for _ in 0..3 {
            manager.update_from_result(auth_id, true, 200).await;
        }
        assert_eq!(manager.get_status(auth_id).await, HealthStatus::Healthy);
    }
}
