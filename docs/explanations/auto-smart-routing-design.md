# Auto Smart Routing Design

This document proposes a detailed, implementable design for automatic smart routing in this repository.

The target is:

- minimal manual tuning,
- strong capability-based routing,
- robust fallback behavior,
- graceful degradation when learning data is sparse,
- compatibility with a localhost single-node gateway,
- a clear path from the current codebase to production-grade routing.

## Design Summary

The routing system should be built as one pipeline:

```text
Request Ingress
  -> Request Classification
  -> Candidate Route Construction
  -> Hard Constraint Filtering
  -> Feature Assembly
  -> Route Utility Estimation
  -> Exploration / Exploitation Selection
  -> Ordered Fallback Plan
  -> Upstream Execution
  -> Outcome Recording
  -> Online Model Update
```

The router should score full route candidates:

`route = (model_id, provider_endpoint, auth_id)`

This is a better unit than auth-only or model-only routing because real outcomes depend on both.

## Goals

- automatically infer route choice from live traffic,
- prioritize capability correctness first,
- improve latency, reliability, and cost over time,
- remain explainable,
- support fallback and safe failure,
- fit the current Rust + Tokio + Axum + SQLite stack.

## Non-Goals

- full reinforcement learning,
- distributed multi-node coordination in v1,
- perfect semantic intent understanding,
- zero configuration for business rules.

Some things should remain explicit:

- tenant allow/deny rules,
- premium-model eligibility,
- compliance restrictions,
- retry ceilings,
- safety guardrails.

## Design Principles

### 1. Capability correctness before optimization

A fast wrong model is still a wrong route.

### 2. Learn behavior, do not hand-tune route weights

Manual configuration should express business policy, not micro-optimization.

### 3. Prefer one routing pipeline over many "strategies"

Time, quota, policy, and adaptive behavior should be stages in one system.

### 4. Always return a route plan, not only one route

The gateway should produce:

- primary route,
- fallback route 1,
- fallback route 2.

### 5. Graceful degradation is mandatory

If the learned model is uncertain or unavailable, the router must fall back to deterministic safe heuristics.

## Priority Plan

### P0

Must exist before the router can be considered real.

- request classification,
- full route candidate construction,
- hard constraint filtering,
- deterministic heuristic scoring,
- ordered fallback plans,
- outcome recording,
- loop / recursion guard,
- shared routing state,
- real passthrough request execution.

### P1

High-value auto-routing behavior.

- contextual route features,
- route-level statistics,
- Thompson Sampling exploration,
- route priors and cold-start inheritance,
- online reward updates,
- decision tracing.

### P2

Production hardening and quality improvements.

- tenant-aware priors,
- adaptive quota pressure modeling,
- first-token latency modeling for streaming,
- route diversity-aware fallback generation,
- reward calibration from rolling SLO and budget history.

### P3

Advanced optimization.

- LinUCB or contextual linear reward model,
- semantic loop detection,
- per-request-family reward tuning,
- active drift detection and prior resets.

## Proposed Package Changes

### `packages/smart-routing`

This package should become the real routing engine.

Add modules:

- `request_context.rs`
- `candidate.rs`
- `constraints.rs`
- `features.rs`
- `reward.rs`
- `policy.rs` or reuse existing policy-aware pieces
- `bandit.rs`
- `planner.rs`
- `outcome.rs`
- `loop_guard.rs`
- `state.rs`

Keep existing modules where useful:

- `health.rs`
- `metrics.rs`
- `weight.rs`
- `sqlite/`

Deprecate the placeholder role of:

- `router.rs`

and replace it with a real orchestrator.

### `apps/gateway`

This app should:

- parse OpenAI-compatible requests,
- build `RouteRequestContext`,
- call the route planner,
- execute primary route,
- fail over through fallback plan,
- record outcome,
- emit route decision traces.

### `packages/model-registry`

This package remains the source of model metadata, but it needs stronger support for routing feasibility.

Add or extend metadata for:

- endpoint family compatibility,
- structured-output support,
- tool-call support level,
- streaming support level,
- modality support beyond current booleans,
- provider family aliases,
- fallback equivalence classes.

### `packages/tracing`

Extend to support:

- route decision spans,
- fallback attempt spans,
- exporter abstraction for Langfuse / OTLP,
- correlation of inbound request and upstream provider attempts.

## Core Domain Types

These types should be introduced in `packages/smart-routing`.

### `RouteRequestContext`

Purpose:

- normalized request facts used by routing.

Fields:

- `request_id: String`
- `tenant_id: Option<String>`
- `requested_model: Option<String>`
- `requested_provider: Option<String>`
- `estimated_input_tokens: usize`
- `estimated_output_tokens: usize`
- `total_estimated_tokens: usize`
- `requires_streaming: bool`
- `requires_tools: bool`
- `requires_vision: bool`
- `requires_thinking: bool`
- `latency_sensitivity: LatencySensitivity`
- `budget_sensitivity: BudgetSensitivity`
- `quality_preference: QualityPreference`
- `request_family: RequestFamily`
- `time_bucket: TimeBucket`
- `metadata: HashMap<String, String>`

### `CapabilityRequirements`

Purpose:

- explicit hard and soft capability requirements.

Fields:

- `hard: Vec<CapabilityRequirement>`
- `soft: Vec<CapabilityRequirement>`

### `RouteCandidate`

Purpose:

- one feasible route before final selection.

Fields:

- `route_id: RouteId`
- `model: ModelInfo`
- `provider_endpoint: ProviderEndpoint`
- `auth_id: String`
- `estimated_cost_usd: f64`
- `feature_vector: RouteFeatureVector`
- `feasibility_notes: Vec<String>`

### `RouteId`

Purpose:

- stable key for route statistics and priors.

Fields:

- `model_id: String`
- `provider: String`
- `endpoint_variant: String`
- `auth_id: String`

### `RouteDecision`

Purpose:

- explainable result of route planning.

Fields:

- `primary: RouteCandidate`
- `fallbacks: Vec<RouteCandidate>`
- `decision_reason: RouteDecisionReason`
- `planner_mode: PlannerMode`
- `candidate_count: usize`
- `rejected_candidates: Vec<RejectedCandidate>`

### `RouteOutcome`

Purpose:

- normalized execution result for online updates.

Fields:

- `route_id: RouteId`
- `success: bool`
- `status_code: Option<u16>`
- `latency_ms: u64`
- `first_token_latency_ms: Option<u64>`
- `input_tokens: Option<u32>`
- `output_tokens: Option<u32>`
- `cost_usd: Option<f64>`
- `retry_count: u32`
- `fallback_used: bool`
- `loop_guard_triggered: bool`
- `error_class: OutcomeErrorClass`

## Routing Pipeline Design

### Stage 1: Request Classification

Priority: P0

The gateway should convert raw API requests into `RouteRequestContext`.

Input:

- chat completions requests,
- responses API requests,
- future provider-native request formats.

Inference rules:

- any image input or multimodal content -> `requires_vision = true`
- tools present -> `requires_tools = true`
- explicit reasoning flag or model family hint -> `requires_thinking = true`
- `stream = true` -> `requires_streaming = true`
- large prompt -> higher context requirement
- background task metadata -> higher budget sensitivity, lower latency sensitivity
- interactive user traffic -> higher latency sensitivity

Implementation:

- create a `RequestClassifier` trait,
- add `OpenAIRequestClassifier` in `apps/gateway`,
- keep classifier logic deterministic in v1,
- avoid LLM-based request classification in the router itself.

### Stage 2: Candidate Route Construction

Priority: P0

The planner should build route candidates from:

- registry models,
- provider endpoint config,
- auth inventory,
- auth-model compatibility mapping.

Inputs required:

- model catalog,
- configured provider endpoints,
- auth credentials,
- auth capability map,
- tenant policy overlay.

Implementation:

- create `CandidateBuilder`,
- output all possible `RouteCandidate`s before filtering,
- use stable `RouteId` so metrics can be stored consistently.

### Stage 3: Hard Constraint Filtering

Priority: P0

A route survives only if all hard constraints pass.

Hard constraints:

- required capabilities supported,
- context fits,
- auth is available,
- auth is not in cooldown,
- endpoint supports stream/tool/vision mode,
- tenant policy allows model/provider,
- provider is not disabled,
- recursion guard passes,
- retry budget not exhausted.

Implementation:

- create `ConstraintFilter`,
- return `AcceptedCandidate` and `RejectedCandidate`,
- rejection reasons must be traceable.

### Stage 4: Feature Assembly

Priority: P1

For each surviving route, build a normalized feature vector.

Feature groups:

- capability fit,
- context headroom,
- estimated cost,
- rolling success probability,
- rolling timeout probability,
- rolling 429 probability,
- rolling 5xx probability,
- p50 latency,
- p95 latency,
- first-token latency,
- quota headroom,
- in-flight concurrency,
- tenant-route historical success,
- time-bucket performance,
- loop risk.

Implementation:

- create `RouteFeatureAssembler`,
- features come from shared in-memory state backed by SQLite,
- every feature must define default behavior for cold start.

### Stage 5: Utility Estimation

Priority: P1

Use a compact reward model:

```text
reward =
  success_value
  - latency_penalty
  - cost_penalty
  - retry_penalty
  - loop_penalty
  - quota_failure_penalty
```

The planner should estimate:

- `p_success`
- `p_timeout`
- `p_rate_limit`
- `expected_latency`
- `expected_cost`
- `p_loop_risk`

Then compute:

```text
estimated_utility =
  a * p_success
  - b * normalized_latency
  - c * normalized_cost
  - d * p_rate_limit
  - e * p_loop_risk
```

Coefficients:

- `a` is large and stable,
- `b` and `c` depend on request class,
- `d` and `e` are safety-heavy,
- coefficients remain global or tenant-level, not per-route hand-tuned.

Implementation:

- start with heuristic coefficient defaults,
- calibrate them with rolling history later,
- store them in config only at the global level.

### Stage 6: Exploration Policy

Priority: P1

Use Thompson Sampling in v1.

Why:

- easy to implement,
- uncertainty-aware,
- handles cold start gracefully,
- simpler than LinUCB for initial rollout.

Per-route learned state:

- success posterior,
- timeout posterior,
- rate-limit posterior,
- loop-risk posterior,
- rolling latency distribution,
- rolling cost distribution.

Sampling approach:

- sample `p_success`,
- sample `p_timeout`,
- sample `p_rate_limit`,
- combine sampled values with deterministic cost and latency estimates,
- choose highest sampled utility.

Implementation:

- create `BanditPolicy` trait,
- add `ThompsonSamplingPolicy`,
- planner uses bandit only after hard constraints and feature assembly.

### Stage 7: Fallback Plan Generation

Priority: P0

The planner must produce diverse fallbacks.

Rules:

- fallback 1 should differ from primary auth,
- fallback 2 should differ from primary provider if possible,
- avoid correlated routes when upstream failures are provider-wide,
- ensure every fallback still satisfies hard constraints.

Implementation:

- create `FallbackPlanner`,
- use score plus diversity penalty,
- diversity dimensions: auth, provider, model family.

### Stage 8: Execution and Outcome Recording

Priority: P0

The gateway executes the primary route and falls back when needed.

Execution loop:

1. select primary route,
2. execute upstream request,
3. if hard-retryable failure occurs, move to next fallback,
4. stop when success or retry budget exhausted,
5. record final outcome and per-attempt outcomes.

Retryable failure classes:

- timeout,
- connection error,
- 429,
- selected 5xx classes.

Non-retryable classes:

- invalid request,
- policy block,
- unsupported capability,
- loop guard block.

Implementation:

- add `RouteExecutor`,
- add `OutcomeRecorder`,
- update health and route statistics synchronously enough to preserve correctness.

## Graceful Degradation Design

This part is critical.

The router must keep working when learned data is missing or stale.

### Planner modes

#### `Learned`

Use full contextual bandit planning.

Use when:

- route state exists,
- feature store is healthy,
- uncertainty is acceptable.

#### `Heuristic`

Use deterministic utility scoring without bandit sampling.

Use when:

- route history is sparse,
- feature store is partially available.

#### `SafeWeighted`

Use capability filter + existing weighted selector behavior.

Use when:

- route-level features unavailable,
- only auth health and latency metrics exist.

#### `DeterministicFallback`

Use simplest safe route order:

- capability-correct,
- healthy,
- lowest estimated failure risk,
- cheapest acceptable.

Use when:

- state store unavailable,
- planner internal failure occurs,
- migration or bootstrap mode.

Implementation:

- define `PlannerMode`,
- every decision records which mode was used,
- this is required for debugging and trust.

## Loop and Runaway Protection

Priority: P0

Loop control should block both routing recursion and repeated ineffective retries.

### Guards

- `max_attempts_per_request`
- `max_same_route_retries`
- `max_same_provider_retries`
- `max_same_output_fingerprint_repeats`
- `self_upstream_block`
- `tool_loop_guard`

### Detection signals

- repeated route ID within one request chain,
- repeated failure class with no new route diversity,
- repeated response fingerprint,
- repeated tool-call name and args fingerprint,
- upstream host resolving to the same gateway instance.

Implementation:

- create `LoopGuard`,
- persist short-lived request-chain state in memory,
- optionally checkpoint fingerprints in SQLite only for debugging.

## Data Storage Design

SQLite is sufficient for v1 and aligns with current architecture.

### New tables

#### `route_stats`

Purpose:

- aggregate route-level performance.

Columns:

- `route_key TEXT PRIMARY KEY`
- `model_id TEXT`
- `provider TEXT`
- `endpoint_variant TEXT`
- `auth_id TEXT`
- `success_count INTEGER`
- `failure_count INTEGER`
- `timeout_count INTEGER`
- `rate_limit_count INTEGER`
- `server_error_count INTEGER`
- `avg_latency_ms REAL`
- `p50_latency_ms REAL`
- `p95_latency_ms REAL`
- `avg_first_token_latency_ms REAL`
- `avg_cost_usd REAL`
- `last_success_at TEXT`
- `last_failure_at TEXT`
- `updated_at TEXT`

#### `route_bucket_stats`

Purpose:

- contextual statistics by route and time/request bucket.

Columns:

- `route_key TEXT`
- `time_bucket TEXT`
- `request_family TEXT`
- `tenant_class TEXT`
- `success_count INTEGER`
- `failure_count INTEGER`
- `avg_latency_ms REAL`
- `avg_cost_usd REAL`
- `updated_at TEXT`

Primary key:

- `(route_key, time_bucket, request_family, tenant_class)`

#### `route_attempts`

Purpose:

- append-only decision and execution history for debugging.

Columns:

- `attempt_id TEXT PRIMARY KEY`
- `request_id TEXT`
- `route_key TEXT`
- `attempt_index INTEGER`
- `selected_by_mode TEXT`
- `sampled_utility REAL`
- `predicted_utility REAL`
- `success INTEGER`
- `status_code INTEGER`
- `latency_ms INTEGER`
- `cost_usd REAL`
- `loop_guard_triggered INTEGER`
- `created_at TEXT`

#### `route_priors`

Purpose:

- cold-start inherited priors.

Columns:

- `prior_key TEXT PRIMARY KEY`
- `provider TEXT`
- `tier TEXT`
- `capability_class TEXT`
- `base_success_prior_alpha REAL`
- `base_success_prior_beta REAL`
- `base_timeout_prior_alpha REAL`
- `base_timeout_prior_beta REAL`
- `base_rate_limit_prior_alpha REAL`
- `base_rate_limit_prior_beta REAL`
- `baseline_latency_ms REAL`
- `baseline_cost_usd REAL`
- `updated_at TEXT`

### State layering

Use two layers:

- in-memory shared state for hot path reads,
- SQLite persistence for durability and startup warmup.

Implementation:

- shared state should live behind `Arc<RwLock<...>>`,
- background flusher persists aggregates,
- append-only attempt writes can happen asynchronously,
- route planner always reads from shared state first.

## Component Design

### `RequestClassifier`

Priority: P0

Trait:

```rust
pub trait RequestClassifier {
    fn classify(&self, request: &GatewayRequest) -> RouteRequestContext;
}
```

### `CandidateBuilder`

Priority: P0

Trait:

```rust
pub trait CandidateBuilder {
    fn build(&self, ctx: &RouteRequestContext) -> Vec<RouteCandidate>;
}
```

### `ConstraintFilter`

Priority: P0

Trait:

```rust
pub trait ConstraintFilter {
    fn filter(
        &self,
        ctx: &RouteRequestContext,
        candidates: Vec<RouteCandidate>,
    ) -> ConstraintFilterResult;
}
```

### `FeatureAssembler`

Priority: P1

Trait:

```rust
pub trait FeatureAssembler {
    fn assemble(
        &self,
        ctx: &RouteRequestContext,
        candidate: &RouteCandidate,
    ) -> RouteFeatureVector;
}
```

### `RewardModel`

Priority: P1

Trait:

```rust
pub trait RewardModel {
    fn estimate(
        &self,
        ctx: &RouteRequestContext,
        features: &RouteFeatureVector,
    ) -> RewardEstimate;
}
```

### `BanditPolicy`

Priority: P1

Trait:

```rust
pub trait BanditPolicy {
    fn select(
        &self,
        ctx: &RouteRequestContext,
        estimates: Vec<RouteEstimate>,
    ) -> RankedRoutePlan;
}
```

### `LoopGuard`

Priority: P0

Trait:

```rust
pub trait LoopGuard {
    fn check_attempt(
        &self,
        request_id: &str,
        route_id: &RouteId,
    ) -> LoopGuardDecision;
}
```

### `OutcomeRecorder`

Priority: P0

Trait:

```rust
pub trait OutcomeRecorder {
    fn record(&self, ctx: &RouteRequestContext, outcome: &RouteOutcome);
}
```

## Detailed Algorithm for AutoRoute-v1

Priority: P1

### Candidate score formula

For each candidate:

```text
base_score =
  sampled_success
  - timeout_risk_penalty
  - rate_limit_penalty
  - normalized_latency_penalty
  - normalized_cost_penalty
  - loop_risk_penalty
```

Where:

- `sampled_success` comes from Thompson Sampling,
- timeout and rate-limit terms come from sampled or rolling posteriors,
- latency and cost are deterministic normalized values,
- loop risk is deterministic in v1.

### Posterior model

Use Beta priors for binary outcomes:

- success / failure,
- timeout / non-timeout,
- 429 / non-429,
- loop-trigger / non-loop-trigger.

For latency:

- maintain EWMA,
- maintain rolling quantiles from aggregated windows,
- do not overcomplicate with a heavy distribution model in v1.

### Normalization

Latency normalization:

- compare candidate latency to the best feasible candidate latency,
- clamp to `[0, 1]`.

Cost normalization:

- compare candidate cost to cheapest feasible route,
- clamp to `[0, 1]`.

This avoids hard-coded global magic thresholds.

### Cold start

If route-local stats are weak:

- load prior from `route_priors`,
- if missing, derive prior from provider + tier + capability class,
- if still missing, use neutral global defaults.

### Fallback ranking

After primary selection:

- re-rank remaining candidates with diversity penalties,
- select two or three fallbacks.

Diversity penalties:

- same auth: high penalty,
- same provider: medium penalty,
- same model family: small penalty.

## Implementation Sequence

### Milestone 1

Priority: P0

Deliver:

- `RouteRequestContext`
- `RouteCandidate`
- `RouteDecision`
- `LoopGuard`
- `CandidateBuilder`
- `ConstraintFilter`
- real passthrough execution path
- deterministic fallback planner

Exit criteria:

- gateway can classify, plan, execute, and fail over safely.

### Milestone 2

Priority: P0

Deliver:

- shared in-memory route state,
- SQLite route tables,
- synchronous-enough outcome recording,
- route attempt tracing.

Exit criteria:

- planner decisions improve from recorded outcomes instead of static config only.

### Milestone 3

Priority: P1

Deliver:

- feature assembler,
- reward model,
- Thompson Sampling route policy,
- cold-start priors.

Exit criteria:

- router learns from traffic with no per-route hand-tuned constants.

### Milestone 4

Priority: P1

Deliver:

- route decision spans,
- fallback attempt spans,
- Langfuse / OTLP exporter abstraction,
- decision audit output.

Exit criteria:

- every route choice is explainable from traces.

### Milestone 5

Priority: P2

Deliver:

- adaptive quota features,
- first-token latency for stream routes,
- tenant-aware priors,
- reward calibration jobs.

Exit criteria:

- router behaves differently and correctly across workload classes.

## Why this design is robust

- hard constraints guarantee capability correctness,
- fallback plans reduce single-route fragility,
- shared state prevents clone divergence,
- contextual learning adapts to drift,
- planner modes allow graceful degradation,
- loop guards prevent runaway cost and recursion.

## Why this design is implementable here

- it reuses the existing model registry,
- it evolves the current smart-routing package instead of replacing it,
- it fits SQLite for local persistence,
- it matches the current Tokio/Axum runtime model,
- it can be shipped incrementally by milestone.

## Immediate Recommendation

The best next implementation target is:

Build P0 and P1 around `AutoRoute-v1`, and do not add more manual strategy flags.

Specifically:

1. replace the placeholder router with a real planner,
2. introduce route-level types and storage,
3. implement request classification and hard filters,
4. add deterministic fallback routing,
5. then add Thompson Sampling on top of that foundation.

That gives the project a real automatic routing core without overengineering the first production version.
