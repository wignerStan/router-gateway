use super::*;
use crate::config::WeightConfig;
use crate::health::HealthStatus;
use crate::metrics::AuthMetrics;

#[cfg(test)]
#[allow(clippy::module_inception)]
mod tests {
    use super::*;

    #[test]
    fn test_weight_calculation() {
        let config = WeightConfig::default();
        let calculator = DefaultWeightCalculator::new(config);

        let auth = AuthInfo {
            id: "test-auth".to_string(),
            priority: Some(0),
            quota_exceeded: false,
            unavailable: false,
            model_states: Vec::new(),
        };

        let metrics = AuthMetrics {
            total_requests: 100,
            success_count: 95,
            failure_count: 5,
            avg_latency_ms: 500.0,
            min_latency_ms: 100.0,
            max_latency_ms: 1000.0,
            success_rate: 0.95,
            error_rate: 0.05,
            consecutive_successes: 10,
            consecutive_failures: 0,
            last_request_time: chrono::Utc::now(),
            last_success_time: Some(chrono::Utc::now()),
            last_failure_time: None,
        };

        let weight = calculator.calculate(&auth, Some(&metrics), HealthStatus::Healthy);
        assert!(weight > 0.0);
        assert!(weight <= 1.0);
    }

    #[test]
    fn test_unhealthy_penalty() {
        let config = WeightConfig::default();
        let calculator = DefaultWeightCalculator::new(config.clone());

        let auth = AuthInfo {
            id: "test-auth".to_string(),
            priority: Some(0),
            quota_exceeded: false,
            unavailable: false,
            model_states: Vec::new(),
        };

        let metrics = AuthMetrics {
            total_requests: 100,
            success_count: 95,
            failure_count: 5,
            avg_latency_ms: 500.0,
            min_latency_ms: 100.0,
            max_latency_ms: 1000.0,
            success_rate: 0.95,
            error_rate: 0.05,
            consecutive_successes: 10,
            consecutive_failures: 0,
            last_request_time: chrono::Utc::now(),
            last_success_time: Some(chrono::Utc::now()),
            last_failure_time: None,
        };

        let healthy_weight = calculator.calculate(&auth, Some(&metrics), HealthStatus::Healthy);
        let unhealthy_weight = calculator.calculate(&auth, Some(&metrics), HealthStatus::Unhealthy);

        assert!(unhealthy_weight < healthy_weight);
        let expected_unhealthy = healthy_weight * config.unhealthy_penalty;
        assert!(
            (unhealthy_weight - expected_unhealthy).abs() < 0.01,
            "unhealthy_weight={} != expected={} (healthy_weight={} * penalty={})",
            unhealthy_weight,
            expected_unhealthy,
            healthy_weight,
            config.unhealthy_penalty
        );
    }

    // ============================================================
    // Edge Case Tests for Weight Calculation Functions
    // ============================================================

    #[test]
    fn test_success_rate_score_null_metrics() {
        let config = WeightConfig::default();
        let calculator = DefaultWeightCalculator::new(config);

        let auth = AuthInfo {
            id: "test-auth".to_string(),
            priority: Some(0),
            quota_exceeded: false,
            unavailable: false,
            model_states: Vec::new(),
        };

        let weight = calculator.calculate(&auth, None, HealthStatus::Healthy);
        assert!(
            weight > 0.0,
            "Weight should be positive even with null metrics"
        );
        assert!(weight <= 1.0, "Weight should be at most 1.0");
    }

    #[test]
    fn test_success_rate_score_with_metrics() {
        let config = WeightConfig::default();
        let calculator = DefaultWeightCalculator::new(config);

        let auth = AuthInfo {
            id: "test-auth".to_string(),
            priority: Some(0),
            quota_exceeded: false,
            unavailable: false,
            model_states: Vec::new(),
        };

        let perfect_metrics = AuthMetrics {
            total_requests: 100,
            success_count: 100,
            failure_count: 0,
            avg_latency_ms: 100.0,
            min_latency_ms: 50.0,
            max_latency_ms: 200.0,
            success_rate: 1.0,
            error_rate: 0.0,
            consecutive_successes: 100,
            consecutive_failures: 0,
            last_request_time: chrono::Utc::now(),
            last_success_time: Some(chrono::Utc::now()),
            last_failure_time: None,
        };

        let perfect_weight =
            calculator.calculate(&auth, Some(&perfect_metrics), HealthStatus::Healthy);

        let zero_metrics = AuthMetrics {
            total_requests: 100,
            success_count: 0,
            failure_count: 100,
            avg_latency_ms: 100.0,
            min_latency_ms: 50.0,
            max_latency_ms: 200.0,
            success_rate: 0.0,
            error_rate: 1.0,
            consecutive_successes: 0,
            consecutive_failures: 100,
            last_request_time: chrono::Utc::now(),
            last_success_time: None,
            last_failure_time: Some(chrono::Utc::now()),
        };

        let zero_weight = calculator.calculate(&auth, Some(&zero_metrics), HealthStatus::Healthy);

        assert!(
            perfect_weight > zero_weight,
            "Perfect success rate should yield higher weight"
        );
    }

    #[test]
    fn test_latency_score_extreme_values() {
        let config = WeightConfig::default();
        let calculator = DefaultWeightCalculator::new(config);

        let auth = AuthInfo {
            id: "test-auth".to_string(),
            priority: Some(0),
            quota_exceeded: false,
            unavailable: false,
            model_states: Vec::new(),
        };

        let low_latency_metrics = AuthMetrics {
            total_requests: 100,
            success_count: 100,
            failure_count: 0,
            avg_latency_ms: 1.0,
            min_latency_ms: 1.0,
            max_latency_ms: 5.0,
            success_rate: 1.0,
            error_rate: 0.0,
            consecutive_successes: 100,
            consecutive_failures: 0,
            last_request_time: chrono::Utc::now(),
            last_success_time: Some(chrono::Utc::now()),
            last_failure_time: None,
        };

        let low_latency_weight =
            calculator.calculate(&auth, Some(&low_latency_metrics), HealthStatus::Healthy);

        let high_latency_metrics = AuthMetrics {
            total_requests: 100,
            success_count: 100,
            failure_count: 0,
            avg_latency_ms: 10000.0,
            min_latency_ms: 5000.0,
            max_latency_ms: 20000.0,
            success_rate: 1.0,
            error_rate: 0.0,
            consecutive_successes: 100,
            consecutive_failures: 0,
            last_request_time: chrono::Utc::now(),
            last_success_time: Some(chrono::Utc::now()),
            last_failure_time: None,
        };

        let high_latency_weight =
            calculator.calculate(&auth, Some(&high_latency_metrics), HealthStatus::Healthy);

        assert!(
            low_latency_weight > high_latency_weight,
            "Lower latency should yield higher weight"
        );

        let zero_latency_metrics = AuthMetrics {
            total_requests: 100,
            success_count: 100,
            failure_count: 0,
            avg_latency_ms: 0.0,
            min_latency_ms: 0.0,
            max_latency_ms: 0.0,
            success_rate: 1.0,
            error_rate: 0.0,
            consecutive_successes: 100,
            consecutive_failures: 0,
            last_request_time: chrono::Utc::now(),
            last_success_time: Some(chrono::Utc::now()),
            last_failure_time: None,
        };

        let zero_latency_weight =
            calculator.calculate(&auth, Some(&zero_latency_metrics), HealthStatus::Healthy);
        assert!(
            zero_latency_weight > 0.0,
            "Zero latency should still produce positive weight"
        );
    }

    #[test]
    fn test_health_score_edge_cases() {
        let config = WeightConfig::default();
        let calculator = DefaultWeightCalculator::new(config);

        let auth = AuthInfo {
            id: "test-auth".to_string(),
            priority: Some(0),
            quota_exceeded: false,
            unavailable: false,
            model_states: Vec::new(),
        };

        let metrics = AuthMetrics {
            total_requests: 100,
            success_count: 95,
            failure_count: 5,
            avg_latency_ms: 500.0,
            min_latency_ms: 100.0,
            max_latency_ms: 1000.0,
            success_rate: 0.95,
            error_rate: 0.05,
            consecutive_successes: 10,
            consecutive_failures: 0,
            last_request_time: chrono::Utc::now(),
            last_success_time: Some(chrono::Utc::now()),
            last_failure_time: None,
        };

        let healthy_weight = calculator.calculate(&auth, Some(&metrics), HealthStatus::Healthy);
        let degraded_weight = calculator.calculate(&auth, Some(&metrics), HealthStatus::Degraded);
        let unhealthy_weight = calculator.calculate(&auth, Some(&metrics), HealthStatus::Unhealthy);

        assert!(
            healthy_weight > degraded_weight,
            "Healthy weight should be greater than degraded"
        );
        assert!(
            degraded_weight > unhealthy_weight,
            "Degraded weight should be greater than unhealthy"
        );

        assert!(
            healthy_weight >= 0.0,
            "Healthy weight should be non-negative"
        );
        assert!(
            degraded_weight >= 0.0,
            "Degraded weight should be non-negative"
        );
        assert!(
            unhealthy_weight >= 0.0,
            "Unhealthy weight should be non-negative"
        );
    }

    #[test]
    fn test_load_score_bounds() {
        let config = WeightConfig::default();
        let calculator = DefaultWeightCalculator::new(config);

        let auth = AuthInfo {
            id: "test-auth".to_string(),
            priority: Some(0),
            quota_exceeded: false,
            unavailable: false,
            model_states: Vec::new(),
        };

        let high_load_metrics = AuthMetrics {
            total_requests: 10000,
            success_count: 9500,
            failure_count: 500,
            avg_latency_ms: 500.0,
            min_latency_ms: 100.0,
            max_latency_ms: 1000.0,
            success_rate: 0.95,
            error_rate: 0.05,
            consecutive_successes: 10,
            consecutive_failures: 0,
            last_request_time: chrono::Utc::now(),
            last_success_time: Some(chrono::Utc::now()),
            last_failure_time: None,
        };

        let high_load_weight =
            calculator.calculate(&auth, Some(&high_load_metrics), HealthStatus::Healthy);

        let low_load_metrics = AuthMetrics {
            total_requests: 10,
            success_count: 9,
            failure_count: 1,
            avg_latency_ms: 500.0,
            min_latency_ms: 100.0,
            max_latency_ms: 1000.0,
            success_rate: 0.9,
            error_rate: 0.1,
            consecutive_successes: 5,
            consecutive_failures: 0,
            last_request_time: chrono::Utc::now(),
            last_success_time: Some(chrono::Utc::now()),
            last_failure_time: None,
        };

        let low_load_weight =
            calculator.calculate(&auth, Some(&low_load_metrics), HealthStatus::Healthy);

        assert!(
            low_load_weight >= 0.0,
            "Low load weight should be non-negative"
        );
        assert!(
            high_load_weight >= 0.0,
            "High load weight should be non-negative"
        );
    }

    #[test]
    fn test_load_score_with_quota_exceeded() {
        let config = WeightConfig::default();
        let calculator = DefaultWeightCalculator::new(config.clone());

        let metrics = AuthMetrics {
            total_requests: 100,
            success_count: 95,
            failure_count: 5,
            avg_latency_ms: 500.0,
            min_latency_ms: 100.0,
            max_latency_ms: 1000.0,
            success_rate: 0.95,
            error_rate: 0.05,
            consecutive_successes: 10,
            consecutive_failures: 0,
            last_request_time: chrono::Utc::now(),
            last_success_time: Some(chrono::Utc::now()),
            last_failure_time: None,
        };

        let auth_normal = AuthInfo {
            id: "test-auth".to_string(),
            priority: Some(0),
            quota_exceeded: false,
            unavailable: false,
            model_states: Vec::new(),
        };

        let auth_quota = AuthInfo {
            id: "test-auth".to_string(),
            priority: Some(0),
            quota_exceeded: true,
            unavailable: false,
            model_states: Vec::new(),
        };

        let normal_weight =
            calculator.calculate(&auth_normal, Some(&metrics), HealthStatus::Healthy);
        let quota_exceeded_weight =
            calculator.calculate(&auth_quota, Some(&metrics), HealthStatus::Healthy);

        assert!(
            quota_exceeded_weight < normal_weight,
            "Quota exceeded should reduce weight"
        );
        let expected = normal_weight * config.quota_exceeded_penalty;
        assert!(
            (quota_exceeded_weight - expected).abs() < 0.01,
            "Quota exceeded weight should be normal * penalty"
        );
    }

    #[test]
    fn test_load_score_with_model_states() {
        let config = WeightConfig::default();
        let calculator = DefaultWeightCalculator::new(config);

        let metrics = AuthMetrics {
            total_requests: 100,
            success_count: 95,
            failure_count: 5,
            avg_latency_ms: 500.0,
            min_latency_ms: 100.0,
            max_latency_ms: 1000.0,
            success_rate: 0.95,
            error_rate: 0.05,
            consecutive_successes: 10,
            consecutive_failures: 0,
            last_request_time: chrono::Utc::now(),
            last_success_time: Some(chrono::Utc::now()),
            last_failure_time: None,
        };

        let auth_available = AuthInfo {
            id: "test-auth".to_string(),
            priority: Some(0),
            quota_exceeded: false,
            unavailable: false,
            model_states: vec![
                ModelState {
                    unavailable: false,
                    quota_exceeded: false,
                },
                ModelState {
                    unavailable: false,
                    quota_exceeded: false,
                },
            ],
        };

        let auth_partial = AuthInfo {
            id: "test-auth".to_string(),
            priority: Some(0),
            quota_exceeded: false,
            unavailable: false,
            model_states: vec![
                ModelState {
                    unavailable: true,
                    quota_exceeded: false,
                },
                ModelState {
                    unavailable: false,
                    quota_exceeded: false,
                },
            ],
        };

        let available_weight =
            calculator.calculate(&auth_available, Some(&metrics), HealthStatus::Healthy);
        let partial_weight =
            calculator.calculate(&auth_partial, Some(&metrics), HealthStatus::Healthy);

        assert!(
            available_weight > partial_weight,
            "All available models should yield higher weight"
        );
    }

    #[test]
    fn test_priority_score_edge_cases() {
        let config = WeightConfig::default();
        let calculator = DefaultWeightCalculator::new(config);

        let metrics = AuthMetrics {
            total_requests: 100,
            success_count: 95,
            failure_count: 5,
            avg_latency_ms: 500.0,
            min_latency_ms: 100.0,
            max_latency_ms: 1000.0,
            success_rate: 0.95,
            error_rate: 0.05,
            consecutive_successes: 10,
            consecutive_failures: 0,
            last_request_time: chrono::Utc::now(),
            last_success_time: Some(chrono::Utc::now()),
            last_failure_time: None,
        };

        let auth_max = AuthInfo {
            id: "test-auth".to_string(),
            priority: Some(100),
            quota_exceeded: false,
            unavailable: false,
            model_states: Vec::new(),
        };

        let auth_min = AuthInfo {
            id: "test-auth".to_string(),
            priority: Some(-100),
            quota_exceeded: false,
            unavailable: false,
            model_states: Vec::new(),
        };

        let auth_none = AuthInfo {
            id: "test-auth".to_string(),
            priority: None,
            quota_exceeded: false,
            unavailable: false,
            model_states: Vec::new(),
        };

        let auth_zero = AuthInfo {
            id: "test-auth".to_string(),
            priority: Some(0),
            quota_exceeded: false,
            unavailable: false,
            model_states: Vec::new(),
        };

        let max_weight = calculator.calculate(&auth_max, Some(&metrics), HealthStatus::Healthy);
        let min_weight = calculator.calculate(&auth_min, Some(&metrics), HealthStatus::Healthy);
        let none_weight = calculator.calculate(&auth_none, Some(&metrics), HealthStatus::Healthy);
        let zero_weight = calculator.calculate(&auth_zero, Some(&metrics), HealthStatus::Healthy);

        assert!(
            max_weight > zero_weight,
            "Max priority should give higher weight than zero"
        );
        assert!(
            zero_weight > min_weight,
            "Zero priority should give higher weight than min"
        );

        assert!(
            none_weight > min_weight,
            "None priority should be higher than min"
        );
        assert!(
            none_weight < max_weight,
            "None priority should be lower than max"
        );

        assert!(max_weight >= 0.0);
        assert!(min_weight >= 0.0);
        assert!(none_weight >= 0.0);
        assert!(zero_weight >= 0.0);
    }

    #[test]
    fn test_weight_clamping_non_negative() {
        let config = WeightConfig {
            success_rate_weight: 1.0,
            latency_weight: 0.0,
            health_weight: 0.0,
            load_weight: 0.0,
            priority_weight: 0.0,
            unhealthy_penalty: 0.0,
            degraded_penalty: 0.0,
            quota_exceeded_penalty: 0.0,
            unavailable_penalty: 0.0,
        };
        let calculator = DefaultWeightCalculator::new(config);

        let auth = AuthInfo {
            id: "test-auth".to_string(),
            priority: Some(0),
            quota_exceeded: false,
            unavailable: false,
            model_states: Vec::new(),
        };

        let metrics = AuthMetrics {
            total_requests: 100,
            success_count: 0,
            failure_count: 100,
            avg_latency_ms: 10000.0,
            min_latency_ms: 5000.0,
            max_latency_ms: 20000.0,
            success_rate: 0.0,
            error_rate: 1.0,
            consecutive_successes: 0,
            consecutive_failures: 100,
            last_request_time: chrono::Utc::now(),
            last_success_time: None,
            last_failure_time: Some(chrono::Utc::now()),
        };

        let weight = calculator.calculate(&auth, Some(&metrics), HealthStatus::Unhealthy);

        assert!(
            weight >= 0.0,
            "Weight should always be non-negative, got: {}",
            weight
        );
    }

    #[test]
    fn test_degraded_penalty_applied() {
        let config = WeightConfig::default();
        let calculator = DefaultWeightCalculator::new(config.clone());

        let auth = AuthInfo {
            id: "test-auth".to_string(),
            priority: Some(0),
            quota_exceeded: false,
            unavailable: false,
            model_states: Vec::new(),
        };

        let metrics = AuthMetrics {
            total_requests: 100,
            success_count: 95,
            failure_count: 5,
            avg_latency_ms: 500.0,
            min_latency_ms: 100.0,
            max_latency_ms: 1000.0,
            success_rate: 0.95,
            error_rate: 0.05,
            consecutive_successes: 10,
            consecutive_failures: 0,
            last_request_time: chrono::Utc::now(),
            last_success_time: Some(chrono::Utc::now()),
            last_failure_time: None,
        };

        let healthy_weight = calculator.calculate(&auth, Some(&metrics), HealthStatus::Healthy);
        let degraded_weight = calculator.calculate(&auth, Some(&metrics), HealthStatus::Degraded);

        assert!(
            degraded_weight < healthy_weight,
            "Degraded weight ({}) should be less than healthy weight ({})",
            degraded_weight,
            healthy_weight
        );

        assert!(degraded_weight > 0.0, "Degraded weight should be positive");
    }

    #[test]
    fn test_unavailable_penalty_applied() {
        let config = WeightConfig::default();
        let calculator = DefaultWeightCalculator::new(config.clone());

        let metrics = AuthMetrics {
            total_requests: 100,
            success_count: 95,
            failure_count: 5,
            avg_latency_ms: 500.0,
            min_latency_ms: 100.0,
            max_latency_ms: 1000.0,
            success_rate: 0.95,
            error_rate: 0.05,
            consecutive_successes: 10,
            consecutive_failures: 0,
            last_request_time: chrono::Utc::now(),
            last_success_time: Some(chrono::Utc::now()),
            last_failure_time: None,
        };

        let auth_available = AuthInfo {
            id: "test-auth".to_string(),
            priority: Some(0),
            quota_exceeded: false,
            unavailable: false,
            model_states: Vec::new(),
        };

        let auth_unavailable = AuthInfo {
            id: "test-auth".to_string(),
            priority: Some(0),
            quota_exceeded: false,
            unavailable: true,
            model_states: Vec::new(),
        };

        let available_weight =
            calculator.calculate(&auth_available, Some(&metrics), HealthStatus::Healthy);
        let unavailable_weight =
            calculator.calculate(&auth_unavailable, Some(&metrics), HealthStatus::Healthy);

        let expected = available_weight * config.unavailable_penalty;
        assert!(
            (unavailable_weight - expected).abs() < 0.01,
            "Unavailable weight should be available * unavailable_penalty"
        );
    }

    #[test]
    fn test_combined_penalties() {
        let config = WeightConfig::default();
        let calculator = DefaultWeightCalculator::new(config.clone());

        let auth = AuthInfo {
            id: "test-auth".to_string(),
            priority: Some(0),
            quota_exceeded: true,
            unavailable: true,
            model_states: Vec::new(),
        };

        let metrics = AuthMetrics {
            total_requests: 100,
            success_count: 95,
            failure_count: 5,
            avg_latency_ms: 500.0,
            min_latency_ms: 100.0,
            max_latency_ms: 1000.0,
            success_rate: 0.95,
            error_rate: 0.05,
            consecutive_successes: 10,
            consecutive_failures: 0,
            last_request_time: chrono::Utc::now(),
            last_success_time: Some(chrono::Utc::now()),
            last_failure_time: None,
        };

        let weight = calculator.calculate(&auth, Some(&metrics), HealthStatus::Unhealthy);

        assert!(
            weight >= 0.0,
            "Weight should be non-negative with combined penalties"
        );

        let base_weight = calculator.calculate(
            &AuthInfo {
                id: "test-auth".to_string(),
                priority: Some(0),
                quota_exceeded: false,
                unavailable: false,
                model_states: Vec::new(),
            },
            Some(&metrics),
            HealthStatus::Healthy,
        );

        let expected = base_weight
            * config.unhealthy_penalty
            * config.quota_exceeded_penalty
            * config.unavailable_penalty;
        assert!(
            (weight - expected).abs() < 0.001,
            "Combined penalties should be multiplicative"
        );
    }

    // ============================================================
    // Edge Case Tests for Weight Calculator - Numerical Stability
    // ============================================================

    #[test]
    fn test_weight_calculation_with_nan_metrics() {
        let config = WeightConfig::default();
        let calculator = DefaultWeightCalculator::new(config);

        let auth = AuthInfo {
            id: "test-auth".to_string(),
            priority: Some(0),
            quota_exceeded: false,
            unavailable: false,
            model_states: Vec::new(),
        };

        let nan_metrics = AuthMetrics {
            total_requests: 100,
            success_count: 50,
            failure_count: 50,
            avg_latency_ms: f64::NAN,
            min_latency_ms: f64::NAN,
            max_latency_ms: f64::NAN,
            success_rate: f64::NAN,
            error_rate: f64::NAN,
            consecutive_successes: 10,
            consecutive_failures: 5,
            last_request_time: chrono::Utc::now(),
            last_success_time: Some(chrono::Utc::now()),
            last_failure_time: None,
        };

        let weight = calculator.calculate(&auth, Some(&nan_metrics), HealthStatus::Healthy);

        assert!(
            !weight.is_nan(),
            "Weight should not be NaN even with NaN metrics"
        );
        assert!(
            weight >= 0.0,
            "Weight should be non-negative even with NaN metrics"
        );
    }

    #[test]
    fn test_weight_calculation_with_inf_metrics() {
        let config = WeightConfig::default();
        let calculator = DefaultWeightCalculator::new(config);

        let auth = AuthInfo {
            id: "test-auth".to_string(),
            priority: Some(0),
            quota_exceeded: false,
            unavailable: false,
            model_states: Vec::new(),
        };

        let inf_metrics = AuthMetrics {
            total_requests: 100,
            success_count: 100,
            failure_count: 0,
            avg_latency_ms: f64::INFINITY,
            min_latency_ms: 0.0,
            max_latency_ms: f64::INFINITY,
            success_rate: 1.0,
            error_rate: 0.0,
            consecutive_successes: 100,
            consecutive_failures: 0,
            last_request_time: chrono::Utc::now(),
            last_success_time: Some(chrono::Utc::now()),
            last_failure_time: None,
        };

        let weight = calculator.calculate(&auth, Some(&inf_metrics), HealthStatus::Healthy);

        assert!(
            weight.is_finite(),
            "Weight should be finite even with Inf metrics"
        );
        assert!(
            (0.0..=1.0).contains(&weight),
            "Weight should be in valid range"
        );
    }

    #[test]
    fn test_priority_weight_extreme_values() {
        let config = WeightConfig::default();
        let calculator = DefaultWeightCalculator::new(config);

        let metrics = AuthMetrics {
            total_requests: 100,
            success_count: 95,
            failure_count: 5,
            avg_latency_ms: 500.0,
            min_latency_ms: 100.0,
            max_latency_ms: 1000.0,
            success_rate: 0.95,
            error_rate: 0.05,
            consecutive_successes: 10,
            consecutive_failures: 0,
            last_request_time: chrono::Utc::now(),
            last_success_time: Some(chrono::Utc::now()),
            last_failure_time: None,
        };

        let auth_max = AuthInfo {
            id: "test-auth".to_string(),
            priority: Some(i32::MAX),
            quota_exceeded: false,
            unavailable: false,
            model_states: Vec::new(),
        };
        let weight_max = calculator.calculate(&auth_max, Some(&metrics), HealthStatus::Healthy);
        assert!(
            weight_max.is_finite() && weight_max > 0.0,
            "Max priority should produce valid weight"
        );

        let auth_min = AuthInfo {
            id: "test-auth".to_string(),
            priority: Some(i32::MIN),
            quota_exceeded: false,
            unavailable: false,
            model_states: Vec::new(),
        };
        let weight_min = calculator.calculate(&auth_min, Some(&metrics), HealthStatus::Healthy);
        assert!(
            weight_min.is_finite() && weight_min >= 0.0,
            "Min priority should produce valid weight"
        );

        assert!(
            weight_max > weight_min,
            "Max priority should give higher weight than min"
        );
    }

    #[test]
    fn test_weight_calculation_zero_metrics() {
        let config = WeightConfig::default();
        let calculator = DefaultWeightCalculator::new(config);

        let auth = AuthInfo {
            id: "test-auth".to_string(),
            priority: Some(0),
            quota_exceeded: false,
            unavailable: false,
            model_states: Vec::new(),
        };

        let zero_metrics = AuthMetrics {
            total_requests: 0,
            success_count: 0,
            failure_count: 0,
            avg_latency_ms: 0.0,
            min_latency_ms: 0.0,
            max_latency_ms: 0.0,
            success_rate: 1.0,
            error_rate: 0.0,
            consecutive_successes: 0,
            consecutive_failures: 0,
            last_request_time: chrono::Utc::now(),
            last_success_time: None,
            last_failure_time: None,
        };

        let weight = calculator.calculate(&auth, Some(&zero_metrics), HealthStatus::Healthy);

        assert!(
            weight > 0.0,
            "Weight should be positive even with zero metrics"
        );
        assert!(weight <= 1.0, "Weight should not exceed 1.0");
    }

    #[test]
    fn test_quota_exceeded_heavy_penalty() {
        let config = WeightConfig::default();
        let calculator = DefaultWeightCalculator::new(config.clone());

        let metrics = AuthMetrics {
            total_requests: 100,
            success_count: 95,
            failure_count: 5,
            avg_latency_ms: 500.0,
            min_latency_ms: 100.0,
            max_latency_ms: 1000.0,
            success_rate: 0.95,
            error_rate: 0.05,
            consecutive_successes: 10,
            consecutive_failures: 0,
            last_request_time: chrono::Utc::now(),
            last_success_time: Some(chrono::Utc::now()),
            last_failure_time: None,
        };

        let auth_normal = AuthInfo {
            id: "test-auth".to_string(),
            priority: Some(0),
            quota_exceeded: false,
            unavailable: false,
            model_states: Vec::new(),
        };

        let auth_quota = AuthInfo {
            id: "test-auth".to_string(),
            priority: Some(0),
            quota_exceeded: true,
            unavailable: false,
            model_states: Vec::new(),
        };

        let normal_weight =
            calculator.calculate(&auth_normal, Some(&metrics), HealthStatus::Healthy);
        let quota_weight = calculator.calculate(&auth_quota, Some(&metrics), HealthStatus::Healthy);

        assert!(
            quota_weight < normal_weight * 0.5,
            "Quota exceeded should reduce weight by at least 50%: normal={}, quota={}",
            normal_weight,
            quota_weight
        );

        let expected = normal_weight * config.quota_exceeded_penalty;
        assert!(
            (quota_weight - expected).abs() < 0.01,
            "Quota penalty should match config"
        );
    }

    #[test]
    fn test_model_state_all_unavailable() {
        let config = WeightConfig::default();
        let calculator = DefaultWeightCalculator::new(config);

        let metrics = AuthMetrics {
            total_requests: 100,
            success_count: 95,
            failure_count: 5,
            avg_latency_ms: 500.0,
            min_latency_ms: 100.0,
            max_latency_ms: 1000.0,
            success_rate: 0.95,
            error_rate: 0.05,
            consecutive_successes: 10,
            consecutive_failures: 0,
            last_request_time: chrono::Utc::now(),
            last_success_time: Some(chrono::Utc::now()),
            last_failure_time: None,
        };

        let auth_all_unavailable = AuthInfo {
            id: "test-auth".to_string(),
            priority: Some(0),
            quota_exceeded: false,
            unavailable: false,
            model_states: vec![
                ModelState {
                    unavailable: true,
                    quota_exceeded: false,
                },
                ModelState {
                    unavailable: true,
                    quota_exceeded: false,
                },
                ModelState {
                    unavailable: true,
                    quota_exceeded: false,
                },
            ],
        };

        let auth_all_available = AuthInfo {
            id: "test-auth".to_string(),
            priority: Some(0),
            quota_exceeded: false,
            unavailable: false,
            model_states: vec![
                ModelState {
                    unavailable: false,
                    quota_exceeded: false,
                },
                ModelState {
                    unavailable: false,
                    quota_exceeded: false,
                },
                ModelState {
                    unavailable: false,
                    quota_exceeded: false,
                },
            ],
        };

        let weight_unavailable =
            calculator.calculate(&auth_all_unavailable, Some(&metrics), HealthStatus::Healthy);
        let weight_available =
            calculator.calculate(&auth_all_available, Some(&metrics), HealthStatus::Healthy);

        assert!(
            weight_available > weight_unavailable,
            "All available models should yield higher weight than all unavailable"
        );
    }

    #[test]
    fn test_planner_mode_selection() {
        let config = WeightConfig::default();
        let calculator = DefaultWeightCalculator::new(config);

        let full_metrics = AuthMetrics {
            total_requests: 100,
            success_count: 95,
            failure_count: 5,
            avg_latency_ms: 500.0,
            min_latency_ms: 100.0,
            max_latency_ms: 1000.0,
            success_rate: 0.95,
            error_rate: 0.05,
            consecutive_successes: 10,
            consecutive_failures: 0,
            last_request_time: chrono::Utc::now(),
            last_success_time: Some(chrono::Utc::now()),
            last_failure_time: None,
        };

        let sparse_metrics = AuthMetrics {
            total_requests: 5,
            success_count: 4,
            failure_count: 1,
            avg_latency_ms: 500.0,
            min_latency_ms: 100.0,
            max_latency_ms: 1000.0,
            success_rate: 0.8,
            error_rate: 0.2,
            consecutive_successes: 2,
            consecutive_failures: 0,
            last_request_time: chrono::Utc::now(),
            last_success_time: Some(chrono::Utc::now()),
            last_failure_time: None,
        };

        let auth = AuthInfo {
            id: "test-auth".to_string(),
            priority: Some(0),
            quota_exceeded: false,
            unavailable: false,
            model_states: Vec::new(),
        };

        let full_weight = calculator.calculate(&auth, Some(&full_metrics), HealthStatus::Healthy);
        let sparse_weight =
            calculator.calculate(&auth, Some(&sparse_metrics), HealthStatus::Healthy);
        let none_weight = calculator.calculate(&auth, None, HealthStatus::Healthy);

        assert!(full_weight > 0.0 && full_weight <= 1.0);
        assert!(sparse_weight > 0.0 && sparse_weight <= 1.0);
        assert!(none_weight > 0.0 && none_weight <= 1.0);
    }
}
