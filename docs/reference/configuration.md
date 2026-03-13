# Configuration Reference

> **Source:** `packages/smart-routing/src/config/mod.rs`
> **Last Updated:** 2026-03-13

This document provides complete reference information for all configuration options in the Gateway smart routing system.

## Overview

The Gateway uses a hierarchical configuration structure defined in Rust structs. Configuration can be provided via:

- Configuration files (JSON, TOML, YAML)
- Environment variables
- Programmatic configuration

---

## SmartRoutingConfig

The root configuration struct for smart routing.

**Source:** `packages/smart-routing/src/config/mod.rs:25`

| Field         | Type               | Default      | Description                             |
| ------------- | ------------------ | ------------ | --------------------------------------- |
| `enabled`     | `bool`             | `true`       | Enable or disable smart routing         |
| `strategy`    | `String`           | `"weighted"` | Routing strategy (see strategies below) |
| `weight`      | `WeightConfig`     | See defaults | Weight calculation configuration        |
| `health`      | `HealthConfig`     | See defaults | Health check configuration              |
| `time_aware`  | `TimeAwareConfig`  | See defaults | Time-aware routing configuration        |
| `quota_aware` | `QuotaAwareConfig` | See defaults | Quota-aware routing configuration       |
| `policy`      | `PolicyConfig`     | See defaults | Policy-aware routing configuration      |
| `log`         | `LogConfig`        | See defaults | Logging configuration                   |

### Routing Strategies

| Strategy       | Description                                         |
| -------------- | --------------------------------------------------- |
| `weighted`     | Weighted selection based on performance metrics     |
| `time_aware`   | Considers time-of-day for credential selection      |
| `quota_aware`  | Balances usage across credentials to respect quotas |
| `adaptive`     | Dynamically selects strategy based on conditions    |
| `policy_aware` | Route based on model-registry policy rules          |

---

## WeightConfig

Configuration for weighted routing calculations.

**Source:** `packages/smart-routing/src/config/mod.rs:83`

| Field                    | Type  | Default | Range   | Description                                       |
| ------------------------ | ----- | ------- | ------- | ------------------------------------------------- |
| `success_rate_weight`    | `f64` | `0.35`  | 0.0-1.0 | Weight for success rate factor                    |
| `latency_weight`         | `f64` | `0.25`  | 0.0-1.0 | Weight for latency factor                         |
| `health_weight`          | `f64` | `0.20`  | 0.0-1.0 | Weight for health status factor                   |
| `load_weight`            | `f64` | `0.15`  | 0.0-1.0 | Weight for current load factor                    |
| `priority_weight`        | `f64` | `0.05`  | 0.0-1.0 | Weight for priority factor                        |
| `unhealthy_penalty`      | `f64` | `0.01`  | 0.0-1.0 | Penalty multiplier for unhealthy credentials      |
| `degraded_penalty`       | `f64` | `0.50`  | 0.0-1.0 | Penalty multiplier for degraded credentials       |
| `quota_exceeded_penalty` | `f64` | `0.10`  | 0.0-1.0 | Penalty multiplier for quota-exceeded credentials |
| `unavailable_penalty`    | `f64` | `0.01`  | 0.0-1.0 | Penalty multiplier for unavailable credentials    |

### Weight Normalization

The system automatically normalizes weights so they sum to 1.0:

```rust
// Weights are normalized if total differs from 1.0
total = success_rate + latency + health + load + priority
if total != 1.0 {
    success_rate /= total
    latency /= total
    health /= total
    load /= total
    priority /= total
}
```

**Source:** `packages/smart-routing/src/config/mod.rs:137`

---

## HealthConfig

Configuration for credential health tracking.

**Source:** `packages/smart-routing/src/config/time_quota.rs:8`

| Field                     | Type                     | Default      | Description                                            |
| ------------------------- | ------------------------ | ------------ | ------------------------------------------------------ |
| `healthy_threshold`       | `i32`                    | `3`          | Consecutive successes to mark credential as healthy    |
| `unhealthy_threshold`     | `i32`                    | `5`          | Consecutive failures to mark credential as unhealthy   |
| `degraded_threshold`      | `f64`                    | `0.30`       | Error rate threshold (0.0-1.0) to enter degraded state |
| `cooldown_period_seconds` | `i64`                    | `60`         | Wait time after failure before retry (seconds)         |
| `status_codes`            | `StatusCodeHealthConfig` | See defaults | HTTP status code health rules                          |

### Health State Transitions

```
          [Success x healthy_threshold]
    ┌────────────────────────────────────┐
    │                                    │
    │                                    ▼
┌───┴───┐                          ┌─────────┐
│Healthy│                          │Degraded │
└───┬───┘                          └────┬────┘
    │                                    │
    │ [Error rate > degraded_threshold]  │ [Success x threshold]
    │                                    │
    ▼                                    ▼
┌───────────┐  [Failure x unhealthy_threshold]  ┌─────────┐
│Unhealthy  │◄──────────────────────────────────│Degraded │
└───────────┘                                   └─────────┘
```

---

## StatusCodeHealthConfig

Configuration for HTTP status code health classification.

**Source:** `packages/smart-routing/src/config/time_quota.rs:23`

| Field       | Type       | Default                          | Description                              |
| ----------- | ---------- | -------------------------------- | ---------------------------------------- |
| `healthy`   | `Vec<i32>` | `[200, 201, 202, 204]`           | Status codes indicating healthy response |
| `degraded`  | `Vec<i32>` | `[429, 503]`                     | Status codes indicating degraded state   |
| `unhealthy` | `Vec<i32>` | `[401, 402, 403, 500, 502, 504]` | Status codes indicating unhealthy state  |

### Status Code Categories

| Category  | Codes        | Interpretation                                       |
| --------- | ------------ | ---------------------------------------------------- |
| Healthy   | 2xx          | Request succeeded                                    |
| Degraded  | 429, 503     | Rate limited or temporarily unavailable              |
| Unhealthy | 401-403, 5xx | Authentication/authorization failure or server error |

---

## TimeAwareConfig

Configuration for time-based routing optimization.

**Source:** `packages/smart-routing/src/config/time_quota.rs:34`

| Field                           | Type                           | Default | Description                             |
| ------------------------------- | ------------------------------ | ------- | --------------------------------------- |
| `enabled`                       | `bool`                         | `false` | Enable time-aware routing               |
| `peak_hours`                    | `Vec<TimeSlot>`                | `[]`    | List of peak hour time slots            |
| `off_peak_factor`               | `f64`                          | `1.2`   | Weight multiplier during off-peak hours |
| `preferred_auths_per_time_slot` | `HashMap<String, Vec<String>>` | `{}`    | Preferred credentials per time slot     |

### TimeSlot

**Source:** `packages/smart-routing/src/config/time_quota.rs:47`

| Field          | Type       | Range | Description                         |
| -------------- | ---------- | ----- | ----------------------------------- |
| `start_hour`   | `i32`      | 0-23  | Start hour of the time slot         |
| `end_hour`     | `i32`      | 0-23  | End hour of the time slot           |
| `days_of_week` | `Vec<i32>` | 0-6   | Days of week (0=Sunday, 6=Saturday) |
| `factor`       | `f64`      | > 0.0 | Weight factor for this time slot    |

### Example Configuration

```json
{
  "time_aware": {
    "enabled": true,
    "peak_hours": [
      {
        "start_hour": 9,
        "end_hour": 17,
        "days_of_week": [1, 2, 3, 4, 5],
        "factor": 1.5
      }
    ],
    "off_peak_factor": 0.8,
    "preferred_auths_per_time_slot": {
      "morning": ["auth-premium-001"],
      "evening": ["auth-standard-001"]
    }
  }
}
```

---

## QuotaAwareConfig

Configuration for quota-balanced routing.

**Source:** `packages/smart-routing/src/config/time_quota.rs:60`

| Field                     | Type     | Default      | Description                                 |
| ------------------------- | -------- | ------------ | ------------------------------------------- |
| `enabled`                 | `bool`   | `false`      | Enable quota-aware routing                  |
| `quota_balance_strategy`  | `String` | `"adaptive"` | Strategy for quota balancing                |
| `reserve_ratio`           | `f64`    | `0.20`       | Fraction of quota reserved for peak periods |
| `recovery_window_seconds` | `i64`    | `3600`       | Quota recovery prediction window (seconds)  |

### Quota Balance Strategies

| Strategy      | Description                                             |
| ------------- | ------------------------------------------------------- |
| `least_used`  | Always select credential with most remaining quota      |
| `round_robin` | Distribute requests evenly across credentials           |
| `adaptive`    | Dynamically adjust based on usage patterns and recovery |

### Reserve Ratio

The `reserve_ratio` determines how much quota to reserve:

- `0.20` = 20% of quota reserved for peak/emergency use
- Higher values = more conservative quota usage
- Lower values = more aggressive usage

---

## LogConfig

Configuration for routing decision logging.

**Source:** `packages/smart-routing/src/config/time_quota.rs:73`

| Field     | Type     | Default  | Description                     |
| --------- | -------- | -------- | ------------------------------- |
| `enabled` | `bool`   | `false`  | Enable routing decision logging |
| `level`   | `String` | `"info"` | Log level for routing decisions |

### Log Levels

| Level   | Description                                    |
| ------- | ---------------------------------------------- |
| `debug` | Detailed routing calculations and scores       |
| `info`  | Routing decisions with credential selection    |
| `warn`  | Only warnings (degraded/unhealthy credentials) |
| `error` | Only errors (no available credentials)         |

---

## PolicyConfig

Configuration for policy-aware routing.

**Source:** `packages/smart-routing/src/config/mod.rs:47`

| Field             | Type                 | Default | Description                                            |
| ----------------- | -------------------- | ------- | ------------------------------------------------------ |
| `enabled`         | `bool`               | `false` | Enable policy-aware routing                            |
| `config_path`     | `Option<String>`     | `None`  | Path to policy configuration file (JSON)               |
| `inline_policies` | `Vec<RoutingPolicy>` | `[]`    | Inline policy definitions (alternative to config file) |
| `cache_enabled`   | `bool`               | `true`  | Cache policy evaluation results for performance        |

---

## Configuration Example

### Complete Configuration (JSON)

```json
{
  "enabled": true,
  "strategy": "weighted",
  "weight": {
    "success_rate_weight": 0.35,
    "latency_weight": 0.25,
    "health_weight": 0.2,
    "load_weight": 0.15,
    "priority_weight": 0.05,
    "unhealthy_penalty": 0.01,
    "degraded_penalty": 0.5,
    "quota_exceeded_penalty": 0.1,
    "unavailable_penalty": 0.01
  },
  "health": {
    "healthy_threshold": 3,
    "unhealthy_threshold": 5,
    "degraded_threshold": 0.3,
    "cooldown_period_seconds": 60,
    "status_codes": {
      "healthy": [200, 201, 202, 204],
      "degraded": [429, 503],
      "unhealthy": [401, 402, 403, 500, 502, 504]
    }
  },
  "time_aware": {
    "enabled": false,
    "peak_hours": [],
    "off_peak_factor": 1.2,
    "preferred_auths_per_time_slot": {}
  },
  "quota_aware": {
    "enabled": false,
    "quota_balance_strategy": "adaptive",
    "reserve_ratio": 0.2,
    "recovery_window_seconds": 3600
  },
  "log": {
    "enabled": false,
    "level": "info"
  },
  "policy": {
    "enabled": false,
    "config_path": null,
    "inline_policies": [],
    "cache_enabled": true
  }
}
```

### Minimal Configuration (JSON)

```json
{
  "enabled": true,
  "strategy": "weighted"
}
```

All omitted fields use their default values.

---

## Validation

The configuration is automatically validated when loaded:

1. **Strategy validation** - Invalid strategies reset to `"weighted"`
2. **Weight range validation** - Weights outside 0.0-1.0 reset to defaults
3. **Threshold validation** - Invalid thresholds reset to defaults
4. **Time slot validation** - Hours clamped to 0-23, invalid days removed
5. **Quota strategy validation** - Invalid strategies reset to `"adaptive"`

**Source:** `packages/smart-routing/src/config/mod.rs:193`

---

## See Also

- [API Reference](./api.md) - HTTP API endpoint documentation
- [API Transformation](../API_TRANSFORMATION.md) - Format conversion architecture
