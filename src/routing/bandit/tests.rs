#![allow(clippy::float_cmp)]
use super::*;
use std::collections::HashMap;

mod route_selection {
    use super::*;

    #[test]
    fn test_bandit_select_route_empty() {
        let policy = BanditPolicy::new();
        let result = policy.select_route(&[]);
        assert!(result.is_none());
    }

    #[test]
    fn test_bandit_select_route_single() {
        let policy = BanditPolicy::new();
        let result = policy.select_route(&["route1"]);
        assert_eq!(result, Some("route1".to_string()));
    }

    #[test]
    fn test_bandit_select_route_multiple() {
        let policy = BanditPolicy::new();
        let routes: Vec<&str> = vec!["route1", "route2", "route3"];

        let result = policy.select_route(&routes);
        assert!(result.is_some());
        assert!(routes.contains(&result.unwrap().as_str()));
    }

    #[test]
    fn test_bandit_exploration_vs_exploitation() {
        let mut policy = BanditPolicy::new();

        // Train route1 to be good
        for _ in 0..20 {
            policy.record_result("route1", true, 0.9);
        }

        // Train route2 to be bad
        for _ in 0..20 {
            policy.record_result("route2", false, 0.2);
        }

        // route3 is unknown (exploration candidate)

        let routes: Vec<&str> = vec!["route1", "route2", "route3"];

        // Run many selections and count
        let mut counts = HashMap::new();
        for _ in 0..100 {
            let selected = policy.select_route(&routes).unwrap();
            *counts.entry(selected).or_insert(0) += 1;
        }

        // route1 should be selected most (exploitation)
        // route3 should get some selections (exploration)
        // route2 should be selected least
        assert!(counts.get("route1").unwrap_or(&0) > counts.get("route2").unwrap_or(&0));
    }
}

mod recording_and_decay {
    use super::*;

    #[test]
    fn test_bandit_record_result() {
        let mut policy = BanditPolicy::new();

        policy.record_result("route1", true, 0.8);
        policy.record_result("route1", true, 0.9);
        policy.record_result("route1", false, 0.3);

        let stats = policy.get_stats("route1").unwrap();
        assert_eq!(stats.successes, 2.0 + 1.0); // 2 successes + prior
        assert_eq!(stats.failures, 1.0 + 1.0); // 1 failure + prior
        assert_eq!(stats.pulls, 3);
        assert_eq!(stats.last_utility, 0.3);
    }

    #[test]
    fn test_sample_decay() {
        let config = BanditConfig {
            sample_decay: 0.9,
            ..Default::default()
        };

        let mut policy = BanditPolicy::with_config(config);

        policy.record_result("route1", true, 0.9);
        policy.record_result("route1", true, 0.9);

        let stats1 = policy.get_stats("route1").unwrap();
        let successes_after_2 = stats1.successes;

        // Add more pulls
        for _ in 0..10 {
            policy.record_result("route1", true, 0.9);
        }

        let stats2 = policy.get_stats("route1").unwrap();

        // With decay, successes should not grow linearly
        assert!(stats2.successes < successes_after_2 + 10.0);
    }

    #[test]
    fn test_record_result_with_zero_utility() {
        let mut policy = BanditPolicy::new();

        policy.record_result("route1", true, 0.0);
        let stats = policy.get_stats("route1").unwrap();

        assert_eq!(stats.last_utility, 0.0);
        assert_eq!(stats.successes, 2.0); // 1 + prior
        assert_eq!(stats.pulls, 1);
    }
}

mod utility_weighting {
    use super::*;

    #[test]
    fn test_bandit_utility_weighting() {
        let mut policy = BanditPolicy::new();

        policy.record_result("route1", true, 0.9);
        policy.record_result("route2", true, 0.9);

        let utilities: HashMap<&str, f64> = [("route1", 0.2), ("route2", 0.9)].into();

        let routes: Vec<&str> = vec!["route1", "route2"];

        let mut count2 = 0;
        for _ in 0..50 {
            if policy.select_route_with_utility(&routes, &utilities) == Some("route2".to_string()) {
                count2 += 1;
            }
        }

        // route2 should be selected more due to higher utility
        assert!(count2 > 25);
    }

    #[test]
    fn test_select_route_with_empty_utilities_map() {
        let policy = BanditPolicy::new();
        let routes: Vec<&str> = vec!["route1", "route2"];

        // Empty utilities map
        let utilities = HashMap::<&str, f64>::new();

        // Should still work, using default utility
        for _ in 0..20 {
            let result = policy.select_route_with_utility(&routes, &utilities);
            assert!(result.is_some());
            assert!(routes.contains(&result.unwrap().as_str()));
        }
    }

    #[test]
    fn test_numerical_stability_extreme_utility_values() {
        let mut policy = BanditPolicy::new();

        // Record with extreme utility values
        policy.record_result("route1", true, f64::MAX / 2.0);
        policy.record_result("route2", true, f64::MIN_POSITIVE);
        policy.record_result("route3", true, 0.0);
        policy.record_result("route4", false, f64::INFINITY);
        policy.record_result("route5", false, f64::NAN);

        // Should not panic and should handle gracefully
        let routes: Vec<&str> = vec!["route1", "route2", "route3", "route4", "route5"];

        // Run selections - should not panic
        for _ in 0..50 {
            let result = policy.select_route(&routes);
            assert!(result.is_some());
        }
    }
}

mod diversity_and_reset {
    use super::*;

    #[test]
    fn test_bandit_diversity_penalty() {
        // Use a higher diversity_weight and lower min_samples to ensure penalty is applied
        let config = BanditConfig {
            diversity_weight: 0.5,       // Higher weight for more pronounced penalty effect
            min_samples_for_thompson: 1, // Ensure penalty is applied after first result
            ..Default::default()
        };
        let mut policy = BanditPolicy::with_config(config);

        // Record enough results to exceed min_samples_for_thompson
        for _ in 0..5 {
            policy.record_result("route1", true, 0.95);
            policy.record_result("route2", true, 0.95);
        }

        // Set high diversity penalty on route2
        policy.set_diversity_penalty("route2", 1.0);

        // route1 should be selected more often due to penalty on route2
        let routes: Vec<&str> = vec!["route1", "route2"];

        let mut count1 = 0;
        // Use larger sample size for statistical significance
        for _ in 0..500 {
            if policy.select_route(&routes) == Some("route1".to_string()) {
                count1 += 1;
            }
        }

        // With penalty of 1.0 and diversity_weight of 0.5, route2 gets a 0.5 penalty
        // This should give route1 a measurable advantage (>55% selection rate)
        assert!(
            count1 > 275,
            "route1 selected {count1} out of 500 times, expected > 275"
        );
    }

    #[test]
    fn test_diversity_penalty_clamping() {
        let mut policy = BanditPolicy::new();

        // Test clamping to [0, 1]
        policy.set_diversity_penalty("route1", -0.5);
        let stats = policy.get_stats("route1").unwrap();
        assert_eq!(
            stats.diversity_penalty, 0.0,
            "Negative penalty should be clamped to 0"
        );

        policy.set_diversity_penalty("route1", 1.5);
        let stats = policy.get_stats("route1").unwrap();
        assert_eq!(
            stats.diversity_penalty, 1.0,
            "Penalty > 1 should be clamped to 1"
        );

        policy.set_diversity_penalty("route1", 0.5);
        let stats = policy.get_stats("route1").unwrap();
        assert_eq!(
            stats.diversity_penalty, 0.5,
            "Penalty in [0,1] should be unchanged"
        );
    }

    #[test]
    fn test_bandit_reset_route() {
        let mut policy = BanditPolicy::new();

        policy.record_result("route1", true, 0.9);
        assert!(policy.get_stats("route1").is_some());

        policy.reset_route("route1");
        assert!(policy.get_stats("route1").is_none());
    }

    #[test]
    fn test_bandit_reset_all() {
        let mut policy = BanditPolicy::new();

        policy.record_result("route1", true, 0.9);
        policy.record_result("route2", false, 0.3);

        assert_eq!(policy.get_all_stats().len(), 2);

        policy.reset_all();
        assert_eq!(policy.get_all_stats().len(), 0);
    }
}

mod beta_sampling {
    use super::*;

    #[test]
    fn test_beta_sampling_bounds() {
        // Test that beta samples are in [0, 1]
        for _ in 0..100 {
            let sample = BanditPolicy::sample_beta(1.0, 1.0);
            assert!((0.0..=1.0).contains(&sample));
        }
    }

    #[test]
    fn test_beta_sampling_distribution() {
        // Beta(1, 1) should be uniform around 0.5
        let mut sum = 0.0_f64;
        for _ in 0..1000 {
            sum += BanditPolicy::sample_beta(1.0, 1.0);
        }
        let mean = sum / 1000.0_f64;

        // Mean should be close to 0.5
        assert!((mean - 0.5).abs() < 0.1);
    }

    #[test]
    fn test_beta_sampling_very_small_alpha_beta() {
        // Very small parameters (near zero)
        for _ in 0..100 {
            let sample = BanditPolicy::sample_beta(0.001, 0.001);
            assert!(
                (0.0..=1.0).contains(&sample),
                "Sample should be in [0,1] with small params: {sample}"
            );
        }
    }

    #[test]
    fn test_beta_sampling_very_large_alpha_beta() {
        // Very large parameters (uses normal approximation)
        let mut samples = Vec::new();
        for _ in 0..100 {
            let sample = BanditPolicy::sample_beta(100.0, 100.0);
            assert!(
                (0.0..=1.0).contains(&sample),
                "Sample should be in [0,1] with large params: {sample}"
            );
            samples.push(sample);
        }

        // Mean should be close to alpha / (alpha + beta) = 0.5
        let mean: f64 = samples.iter().sum::<f64>() / samples.len() as f64;
        assert!(
            (mean - 0.5).abs() < 0.1,
            "Mean should be close to 0.5 with large symmetric params: {mean}"
        );
    }

    #[test]
    fn test_beta_sampling_skewed_distributions() {
        // Beta(100, 1) should give values close to 1
        let mut samples_high = Vec::new();
        for _ in 0..100 {
            samples_high.push(BanditPolicy::sample_beta(100.0, 1.0));
        }
        let mean_high: f64 = samples_high.iter().sum::<f64>() / samples_high.len() as f64;
        assert!(
            mean_high > 0.9,
            "Beta(100,1) mean should be high: {mean_high}"
        );

        // Beta(1, 100) should give values close to 0
        let mut samples_low = Vec::new();
        for _ in 0..100 {
            samples_low.push(BanditPolicy::sample_beta(1.0, 100.0));
        }
        let mean_low: f64 = samples_low.iter().sum::<f64>() / samples_low.len() as f64;
        assert!(mean_low < 0.1, "Beta(1,100) mean should be low: {mean_low}");
    }
}

mod gamma_sampling {
    use super::*;

    #[test]
    fn test_gamma_sampling_positive() {
        // Test that gamma samples are positive
        for _ in 0..100 {
            let sample = BanditPolicy::sample_gamma(2.0);
            assert!(sample > 0.0);
        }
    }

    #[test]
    fn test_gamma_sampling_shape_less_than_one() {
        // Shape < 1 uses transformation method
        for shape in [0.1_f64, 0.5, 0.9] {
            for _ in 0..50 {
                let sample = BanditPolicy::sample_gamma(shape);
                assert!(
                    sample > 0.0,
                    "Gamma sample with shape {shape} should be positive: {sample}"
                );
                assert!(
                    sample.is_finite(),
                    "Gamma sample with shape {shape} should be finite: {sample}"
                );
            }
        }
    }
}

mod priors {
    use super::*;

    #[test]
    fn test_prior_initialization() {
        let policy = BanditPolicy::new();

        // Unknown route should use optimistic prior
        let routes: Vec<&str> = vec!["unknown"];
        let result = policy.select_route(&routes);
        assert_eq!(result, Some("unknown".to_string()));
    }

    #[test]
    fn test_bandit_with_custom_prior() {
        let config = BanditConfig {
            prior_successes: 10.0,
            prior_failures: 2.0,
            min_samples_for_thompson: 100, // Always use prior
            ..Default::default()
        };
        let mut policy = BanditPolicy::with_config(config);

        // Record some results but not enough to exceed min_samples
        policy.record_result("route1", true, 0.9);
        policy.record_result("route1", true, 0.9);

        // Should still use optimistic prior (10,2)
        let mut samples = Vec::new();
        for _ in 0..100 {
            samples.push(policy.thompson_sample("route1"));
        }

        let mean: f64 = samples.iter().sum::<f64>() / samples.len() as f64;
        // Prior mean is 10/(10+2) = 0.833
        assert!(
            mean > 0.7,
            "Mean with optimistic prior (10,2) should be high: {mean}"
        );
    }

    #[test]
    fn test_thompson_sample_with_zero_pulls() {
        let policy = BanditPolicy::new();

        // Unknown route should use prior
        let sample = policy.thompson_sample("unknown-route");

        // With prior (1,1), sample should be in [0,1]
        assert!(
            (0.0..=1.0).contains(&sample),
            "Thompson sample for unknown route should use prior"
        );
    }

    #[test]
    fn test_tier_priors_flagship_higher_than_fast() {
        // Flagship tier should have higher mean prior than fast tier
        let tier_priors = TierPriors::default();
        let (f_alpha, f_beta) = tier_priors.flagship;
        let (s_alpha, s_beta) = tier_priors.fast;

        let flagship_mean = f_alpha / (f_alpha + f_beta);
        let fast_mean = s_alpha / (s_alpha + s_beta);

        assert!(
            flagship_mean > fast_mean,
            "Flagship prior mean ({flagship_mean}) should be > fast prior mean ({fast_mean})"
        );
    }
}

mod tier_priors {
    use super::*;

    #[test]
    fn test_tier_priors_distribution_shapes_differ() {
        let config = BanditConfig {
            tier_priors: Some(TierPriors::default()),
            min_samples_for_thompson: 100, // Always use priors
            ..Default::default()
        };
        let mut policy = BanditPolicy::with_config(config);

        policy.set_route_tier("flagship-route", Tier::Flagship);
        policy.set_route_tier("fast-route", Tier::Fast);

        // Sample many times and compare means
        let flagship_samples: Vec<f64> = (0..500)
            .map(|_| policy.thompson_sample("flagship-route"))
            .collect();
        let fast_samples: Vec<f64> = (0..500)
            .map(|_| policy.thompson_sample("fast-route"))
            .collect();

        let flagship_mean: f64 =
            flagship_samples.iter().sum::<f64>() / flagship_samples.len() as f64;
        let fast_mean: f64 = fast_samples.iter().sum::<f64>() / fast_samples.len() as f64;

        // Flagship should have significantly higher samples than fast
        assert!(
            flagship_mean > fast_mean + 0.2,
            "Flagship mean ({flagship_mean}) should be at least 0.2 higher than fast mean ({fast_mean})"
        );
    }

    #[test]
    fn test_tier_priors_none_uses_default() {
        // Without tier_priors configured, all routes use default priors
        let config = BanditConfig {
            tier_priors: None,
            prior_successes: 3.0,
            prior_failures: 1.0,
            min_samples_for_thompson: 100,
            ..Default::default()
        };
        let mut policy = BanditPolicy::with_config(config);

        policy.set_route_tier("flagship-route", Tier::Flagship);
        policy.set_route_tier("fast-route", Tier::Fast);

        // Both should sample from Beta(3,1) regardless of tier
        let flagship_samples: Vec<f64> = (0..200)
            .map(|_| policy.thompson_sample("flagship-route"))
            .collect();
        let fast_samples: Vec<f64> = (0..200)
            .map(|_| policy.thompson_sample("fast-route"))
            .collect();

        let flagship_mean: f64 =
            flagship_samples.iter().sum::<f64>() / flagship_samples.len() as f64;
        let fast_mean: f64 = fast_samples.iter().sum::<f64>() / fast_samples.len() as f64;

        // Both means should be close to 3/(3+1) = 0.75
        assert!(
            (flagship_mean - 0.75).abs() < 0.15,
            "Flagship mean ({flagship_mean}) should be near 0.75 without tier priors"
        );
        assert!(
            (fast_mean - 0.75).abs() < 0.15,
            "Fast mean ({fast_mean}) should be near 0.75 without tier priors"
        );
    }

    #[test]
    fn test_tier_priors_no_tier_set_uses_default() {
        // Route without tier set falls back to default priors
        let config = BanditConfig {
            tier_priors: Some(TierPriors::default()),
            prior_successes: 2.0,
            prior_failures: 2.0,
            min_samples_for_thompson: 100,
            ..Default::default()
        };
        let policy = BanditPolicy::with_config(config);

        // "unknown-tier-route" has no tier set
        let samples: Vec<f64> = (0..200)
            .map(|_| policy.thompson_sample("unknown-tier-route"))
            .collect();
        let mean: f64 = samples.iter().sum::<f64>() / samples.len() as f64;

        // Should use default prior (2,2) -> mean = 0.5
        assert!(
            (mean - 0.5).abs() < 0.15,
            "Untiered route mean ({mean}) should be near 0.5 (default prior)"
        );
    }

    #[test]
    fn test_tier_priors_after_enough_samples_uses_stats() {
        // After enough samples, actual stats override priors
        let config = BanditConfig {
            tier_priors: Some(TierPriors::default()),
            min_samples_for_thompson: 3,
            ..Default::default()
        };
        let mut policy = BanditPolicy::with_config(config);

        policy.set_route_tier("flagship-route", Tier::Flagship);

        // Record failures only (contradicts flagship's optimistic prior)
        for _ in 0..10 {
            policy.record_result("flagship-route", false, 0.1);
        }

        let samples: Vec<f64> = (0..200)
            .map(|_| policy.thompson_sample("flagship-route"))
            .collect();
        let mean: f64 = samples.iter().sum::<f64>() / samples.len() as f64;

        // After many failures, samples should be low despite flagship prior
        assert!(
            mean < 0.5,
            "After many failures, mean ({mean}) should be low despite flagship prior"
        );
    }
}

mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        #[test]
        fn beta_sample_always_in_unit_interval(
            alpha in 0.001_f64..1000.0,
            beta in 0.001_f64..1000.0,
        ) {
            let sample = BanditPolicy::sample_beta(alpha, beta);
            prop_assert!(sample >= 0.0 && sample <= 1.0,
                "Beta({}, {}) = {}, expected [0, 1]", alpha, beta, sample);
        }

        #[test]
        fn gamma_sample_always_positive_and_finite(
            shape in 0.001_f64..100.0,
        ) {
            let sample = BanditPolicy::sample_gamma(shape);
            prop_assert!(sample > 0.0 && sample.is_finite(),
                "Gamma({}) = {}, expected positive finite", shape, sample);
        }

        #[test]
        fn record_result_never_panics(
            utility in prop_oneof![
                Just(f64::NAN),
                Just(f64::INFINITY),
                Just(f64::NEG_INFINITY),
                Just(0.0_f64),
                Just(-0.0_f64),
                any::<f64>(),
            ],
            success: bool,
        ) {
            let mut policy = BanditPolicy::new();
            policy.record_result("route", success, utility);
            // If we get here, no panic occurred
        }
    }
}
