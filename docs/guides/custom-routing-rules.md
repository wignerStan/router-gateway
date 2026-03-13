# Custom Routing Rules

This guide explains how to configure smart routing strategies in the gateway.

## Available Strategies

| Strategy       | Description                                                       |
| -------------- | ----------------------------------------------------------------- |
| `weighted`     | Default. Selects credentials based on weighted performance scores |
| `time_aware`   | Adjusts routing based on time of day (peak/off-peak)              |
| `quota_aware`  | Balances load across credentials to optimize quota usage          |
| `adaptive`     | Combines multiple strategies with dynamic adjustment              |
| `policy_aware` | Route based on model-registry policy rules                        |

## Basic Configuration

```rust
use smart_routing::config::SmartRoutingConfig;

let mut config = SmartRoutingConfig {
    enabled: true,
    strategy: "weighted".to_string(),
    weight: WeightConfig::default(),
    ..Default::default()
};
config.validate().unwrap();
```

## Adjusting Weights

Control how factors influence credential selection:

```rust
WeightConfig {
    success_rate_weight: 0.35,  // Historical success rate
    latency_weight: 0.25,       // Response latency
    health_weight: 0.20,        // Current health status
    load_weight: 0.15,          // Current request load
    priority_weight: 0.05,      // Manual priority setting
    unhealthy_penalty: 0.01,    // Multiplier when unhealthy
    degraded_penalty: 0.5,      // Multiplier when degraded
    quota_exceeded_penalty: 0.1, // Multiplier when quota exceeded
    unavailable_penalty: 0.01,  // Multiplier when unavailable
}
```

## Time-Aware Routing

```rust
TimeAwareConfig {
    enabled: true,
    peak_hours: vec![TimeSlot {
        start_hour: 9,
        end_hour: 17,
        days_of_week: vec![1, 2, 3, 4, 5], // Mon-Fri
        factor: 1.5,
    }],
    off_peak_factor: 1.2,
    ..Default::default()
}
```

## Quota-Aware Routing

```rust
QuotaAwareConfig {
    enabled: true,
    quota_balance_strategy: "adaptive".to_string(), // least_used, round_robin, adaptive
    reserve_ratio: 0.2,
    recovery_window_seconds: 3600,
}
```

## Health Thresholds

```rust
HealthConfig {
    healthy_threshold: 3,       // Successes to recover
    unhealthy_threshold: 5,     // Failures to mark unhealthy
    degraded_threshold: 0.3,    // Error rate threshold
    cooldown_period_seconds: 60,
    ..Default::default()
}
```

## Example: Latency-Focused Setup

```rust
WeightConfig {
    success_rate_weight: 0.20,
    latency_weight: 0.45,  // Prioritize low latency
    health_weight: 0.20,
    load_weight: 0.10,
    priority_weight: 0.05,
    ..Default::default()
}
```
