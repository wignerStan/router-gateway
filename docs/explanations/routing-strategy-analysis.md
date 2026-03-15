# Routing Strategy Analysis

This report analyzes the repository as it exists today and evaluates how close it is to the stated goal: a localhost LLM gateway that primarily acts as a passthrough router, while making better routing decisions based on time, model capabilities, and model fit.

## Scope

This analysis is based on the current implementation in:

- `crates/gateway/src/main.rs`
- `crates/smart-routing/src/*.rs`
- `crates/model-registry/src/*.rs`
- `crates/llm-tracing/src/*.rs`
- `config/policies.json`

The goal of this report is not to restate the intended design. It is to separate:

1. what is already implemented,
2. what is partially implemented,
3. what is still only documented or implied,
4. what should change next.

## Executive Summary

The repository is a good foundation for a smart LLM router, but it is not yet a real passthrough localhost gateway.

Today, the strongest parts are:

- a usable model metadata layer,
- a reusable weighted credential selector,
- policy matching for model classification,
- a basic Axum tracing middleware,
- a SQLite-backed persistence path for metrics and health.

The weakest parts are:

- the gateway app does not proxy or transform real LLM traffic yet,
- the exported `smart_routing::Router` is still a placeholder,
- `time_aware`, `quota_aware`, and `adaptive` are configuration concepts rather than executed strategies,
- tracing is development-grade and not integrated with Langfuse or an OTEL-compatible backend,
- loop detection / model-output safety controls do not exist,
- there are some state-sharing and behavior gaps that would prevent routing quality from improving reliably under production traffic.

The central conclusion is:

The repo already contains selection primitives, but it still needs a real request-routing pipeline and a clearer strategy execution model before it can be considered a serious localhost gateway router.

## Current State by Subsystem

### 1. Gateway Surface

Current status: scaffold only.

What exists:

- `crates/gateway/src/main.rs` starts an Axum server.
- It wires model registry, smart router, and tracing middleware into shared state.
- It exposes `GET /`, `GET /health`, `GET /api/models`, and `GET /api/route`.

What is missing:

- no `POST /v1/chat/completions`, `POST /v1/responses`, or similar OpenAI-compatible passthrough endpoints,
- no upstream HTTP client,
- no provider credential store,
- no request transformation layer,
- no streaming proxy support,
- no fallback retry chain,
- no decision logging attached to real routing outcomes.

Practical assessment:

The app shell is ready, but the gateway is not yet performing gateway work.

### 2. Model Registry

Current status: real foundation, but mostly static and cache-centric.

What exists:

- `ModelInfo` captures provider, pricing, context, capability, and rate-limit metadata.
- `Registry` provides async fetch, coalesced concurrent loads, TTL caching, and background refresh support.
- `StaticFetcher` seeds a small set of well-known models.
- categorization helpers support tier, cost, context window, provider, and capability-based reasoning.

What is good:

- The registry is one of the cleanest parts of the codebase.
- The type design is good enough to support request-time routing decisions.
- The policy system is aligned with the project goal of capability-driven routing.

What is limited:

- default model data is static and small,
- `Registry` is cache-first, so list/filter operations depend on models already being loaded or refreshed,
- modality coverage is incomplete because `ModelInfo` only tracks a small set of booleans,
- there is no first-class concept of model aliases, endpoint compatibility, or provider-specific request features.

Practical assessment:

This is a strong metadata core, but it needs richer runtime data and better request-facing APIs.

### 3. Smart Routing

Current status: partially implemented.

What exists:

- `DefaultWeightCalculator` computes a score from success rate, latency, health, load, and manual priority.
- `HealthManager` tracks health transitions and cooldowns.
- `MetricsCollector` records success/failure and latency.
- `SmartSelector` filters auths, calculates weights, and makes weighted-random selections.
- `SQLiteSelector` persists metrics and health state via SQLite.

What is good:

- The weighted approach is reasonable for credential selection.
- Health and latency are the correct baseline dimensions.
- SQLite support is appropriate for a local-first gateway.

What is incomplete:

- strategy names such as `time_aware`, `quota_aware`, and `adaptive` exist in config, but selector execution still falls back to the same default weight calculator,
- the exported `Router` type is empty, so the package-level API suggests more than the package currently delivers,
- selection is credential-centric, but the system still lacks a full request-to-model-to-auth routing pipeline,
- `AuthInfo` is too thin to support more advanced routing such as provider-region awareness, real quotas, concurrency budgets, or per-model auth constraints.

Practical assessment:

The repo has a selector, not yet a complete routing engine.

### 4. Policy and Capability Routing

Current status: conceptually strong, operationally partial.

What exists:

- `RoutingPolicy`, `PolicyRegistry`, and `PolicyMatcher`,
- filters across capabilities, tier, cost, context window, provider, and modalities,
- conditions across time of day, day of week, token count, tenant, model family, and metadata,
- JSON-configurable policy templates in `config/policies.json`.

What is good:

- This is the right direction for model-capability-based routing.
- The project is correctly moving away from simple model-name matching.
- The structure is expressive enough for business policies and tenant policies.

What is limited:

- policies act on models, not on the real request path through the gateway,
- policy evaluation does not yet connect to request parsing, intent extraction, or actual upstream routing,
- several modality assumptions are placeholders (`audio`, `embedding`, and parts of `video` support are inferred or hardcoded),
- the system currently optimizes model preference more than end-to-end route feasibility.

Practical assessment:

The policy layer is promising, but it still needs to be connected to a real request contract.

### 5. Tracing and Observability

Current status: internal middleware only.

What exists:

- `TraceSpan`,
- `TraceCollector` abstraction,
- in-memory collector,
- Axum middleware that attaches request IDs, provider/model metadata, latency, and status code.

What is good:

- clean abstraction,
- sane default for local development,
- safe handling of auth headers by not logging secrets.

What is missing:

- no Langfuse integration,
- no OTLP / OpenTelemetry exporter,
- no span model for route decisions, retries, fallback, or policy matches,
- no body-level extraction of prompts, messages, tool usage, or request parameters,
- no trace correlation across inbound request, routing decision, upstream provider request, and response normalization,
- no long-term storage or query path.

Practical assessment:

The tracing package is enough for local debugging, but not for production observability.

### 6. Model Output Detection / Loop Prevention

Current status: not implemented.

There is no current subsystem for:

- loop detection,
- recursive self-routing prevention,
- repeated-output detection,
- repeated-tool-call detection,
- token-budget runaway protection,
- semantic duplicate-response blocking,
- route-attempt ceilings.

Practical assessment:

This is a true gap, not just an enhancement.

## Strategy Analysis

### A. Weighted Routing Strategy

Assessment: valid baseline, but not enough as the primary router strategy.

Why it works:

- It is simple.
- It is explainable.
- It naturally incorporates live metrics.
- It fits local deployments where infrastructure is small and dynamic behavior matters.

Why it is not enough on its own:

- It selects credentials, not complete routes.
- It does not explicitly reason about request intent.
- It does not reason about request complexity, token pressure, or provider-specific strengths.
- It cannot express deterministic requirements such as "must support vision" or "must support tools" unless pre-filtering is done correctly beforehand.

Recommendation:

Keep weighted routing as the final tie-breaker stage after hard constraints and policy filters, not as the whole routing strategy.

### B. Time-Based Routing Strategy

Assessment: currently more aspirational than real.

The repo already has:

- `TimeAwareConfig`,
- peak hours,
- off-peak factor,
- time-slot preferred auths.

But the current selector does not execute a separate time-aware algorithm.

Time-based routing should matter in this project because:

- local users often have predictable workloads,
- some upstream credentials are better kept for peak periods,
- providers can differ meaningfully by time-of-day quotas and latency,
- cost policies often differ between business hours and batch windows.

Recommendation:

Implement time-based routing as a deterministic adjustment stage in the routing pipeline:

1. apply request hard constraints,
2. apply policy constraints,
3. apply time-window boosts/penalties,
4. apply weighted selection among remaining candidates.

This keeps the design simple and avoids creating a separate algorithm that duplicates the same logic.

### C. Model Capability-Based Routing Strategy

Assessment: this should become the core identity of the gateway.

This repo is strongest when it treats routing as a capability matching problem:

- vision request -> vision-capable models only,
- tool-heavy request -> strong tools-capable models preferred,
- long-context request -> large/ultra context models only,
- premium reasoning request -> thinking-capable models preferred,
- low-cost background request -> economy/fast tiers preferred.

The registry and policy code already support this direction.

What is still missing is request interpretation:

- parse incoming request shape,
- infer required capabilities,
- estimate total token load,
- derive strict constraints and soft preferences,
- then route accordingly.

Recommendation:

Make request classification the first-class input to routing. The gateway should route from a `RouteRequestContext`, not directly from loosely assembled auth lists.

### D. Quota-Aware and Adaptive Strategy

Assessment: important for the product, but not yet implemented as real behavior.

The current configuration suggests future support for:

- reserve ratios,
- least-used or round-robin balancing,
- adaptive quota management.

That is directionally correct, but these strategies need actual runtime state:

- request concurrency,
- tokens-per-minute burn,
- requests-per-minute burn,
- quota reset projections,
- recent 429 patterns by auth and provider,
- scheduled reserve capacity.

Recommendation:

Do not implement "adaptive" as a monolithic special mode. Build it from explicit signals:

- quota pressure,
- cooldown state,
- recent latency,
- recent error class,
- reserved capacity,
- tenant priority.

Then adaptive behavior becomes a composition of clear scoring and filtering rules rather than an opaque strategy branch.

## Architectural Risks and Gaps

### 1. The gateway package overstates current capability

The HTTP app looks like a gateway, but it does not yet proxy traffic or execute actual routing.

Impact:

- documentation and implementation are materially out of sync,
- users can overestimate production readiness,
- later refactors become harder because the public story is ahead of the actual interface.

### 2. Strategy configuration is ahead of strategy execution

`SmartRoutingConfig` includes multiple strategies, but selector execution is effectively still weighted-only.

Impact:

- misleading configuration surface,
- false sense of feature completeness,
- difficult debugging because config values may appear meaningful while having no runtime effect.

### 3. State sharing in in-memory routing is fragile

`MetricsCollector` and `HealthManager` clones create fresh in-memory maps, and `SmartSelector::record_result` updates cloned instances in a spawned task instead of guaranteed shared state.

Impact:

- routing quality can stagnate because observed outcomes are not reliably fed back into future selection,
- health state and metrics can diverge across selector copies,
- test behavior may look better than real runtime behavior depending on usage pattern.

This is one of the highest-priority implementation issues in the current codebase.

### 4. Policy routing is model-aware, not route-aware

Policies reason about models well, but real gateway routing must also reason about:

- which auth can access which model,
- provider-specific endpoint compatibility,
- tenant quotas,
- geo or compliance restrictions,
- fallback models,
- streaming support,
- tool-call support parity.

Impact:

- model scoring alone is insufficient for reliable route selection.

### 5. Observability is not yet decision-centric

The tracing package records request metadata and latency, but not the internal route decision lifecycle.

Impact:

- difficult postmortems,
- difficult tuning of weights and policies,
- difficult Langfuse or analytics integration later.

### 6. No loop / runaway protection

A passthrough LLM gateway should protect itself against:

- recursive provider calls,
- repeated model retries with no new evidence,
- tool loops,
- duplicate response segments,
- excessive re-entrant routing in multi-hop workflows.

Impact:

- uncontrolled cost growth,
- confusing user behavior,
- degraded local reliability.

## Proposed Target Architecture

The simplest target architecture is not "many routing strategies". It is one routing pipeline with well-defined stages.

```text
Inbound Request
  -> Parse + Normalize
  -> Build RouteRequestContext
  -> Hard Constraint Filter
  -> Policy Evaluation
  -> Time / Quota / Tenant Adjustments
  -> Weighted Candidate Scoring
  -> Route Selection
  -> Upstream Proxy Execution
  -> Response Validation / Loop Guards
  -> Trace + Metrics + Health Update
```

### Recommended core types

Introduce explicit routing domain objects:

- `RouteRequestContext`
- `CandidateModel`
- `CandidateAuth`
- `RouteCandidate`
- `RouteDecision`
- `RouteDecisionReason`

This is better than stretching `AuthInfo` because it separates:

- request requirements,
- model feasibility,
- credential feasibility,
- final score,
- auditability.

## Improvement Plan

### Phase 1: Turn the gateway into a real passthrough proxy

Priority: highest

Deliver:

- OpenAI-compatible passthrough endpoints,
- upstream HTTP client,
- request/response transformation layer,
- streaming proxy support,
- auth/provider configuration,
- route decision object returned internally and logged.

Success criteria:

- a local client can point to `localhost`,
- the gateway forwards a real request upstream,
- routing chooses an auth/model path before forwarding,
- the gateway updates health/metrics from actual outcomes.

### Phase 2: Refactor routing around a pipeline, not a placeholder router

Priority: highest

Deliver:

- replace empty `smart_routing::Router` with a real orchestrator,
- create `RouteRequestContext`,
- split routing into filter, score, and select phases,
- make weighted routing the final selection stage,
- unify in-memory and SQLite-backed selectors behind a shared interface.

Success criteria:

- the public package API reflects real behavior,
- configuration knobs map to concrete execution stages,
- state updates are deterministic and shared.

### Phase 3: Make capability-based routing first-class

Priority: high

Deliver:

- request capability inference,
- token estimation on inbound requests,
- model fit filtering before auth scoring,
- fallback model chains by capability/tier/cost,
- explicit support matrix for streaming, tools, vision, thinking, and context.

Success criteria:

- routing begins with "what does this request need?" instead of "which auth seems healthiest?".

### Phase 4: Implement real time/quota/adaptive behavior

Priority: high

Deliver:

- time-window score modifiers,
- quota pressure tracking,
- request and token burn rates by auth,
- reserve capacity logic for peak periods,
- adaptive scoring from explicit input signals.

Success criteria:

- `time_aware`, `quota_aware`, and `adaptive` become observable runtime behaviors,
- route decisions can explain why time or quota changed the winner.

### Phase 5: Add production-grade observability

Priority: high

Deliver:

- route-decision spans,
- upstream request spans,
- retry/fallback spans,
- exporter abstraction for Langfuse and OTLP,
- request-body parsing for model, tokens, tools, and streaming metadata,
- trace correlation across inbound and upstream traffic.

Success criteria:

- every failed or surprising routing decision can be reconstructed from traces.

### Phase 6: Add loop and runaway protection

Priority: high

Deliver:

- max route-attempt limit per request,
- retry budget with reason tracking,
- duplicate output fingerprinting,
- repeated tool-call detection,
- response similarity thresholding for repeated completions,
- recursion guard when the gateway is accidentally targeted as its own upstream.

Success criteria:

- the gateway fails closed instead of spiraling on bad retries or recursive paths.

## Specific Improvement Recommendations

### Recommendation 1: Collapse strategy modes into composable routing stages

Do not keep growing top-level strategy branches. Treat:

- policy,
- time,
- quota,
- tenant,
- weight

as composable adjustments in one pipeline.

Why:

- simpler mental model,
- easier testing,
- less duplicated logic,
- more transparent decisions.

### Recommendation 2: Introduce shared state explicitly

Move in-memory metrics and health state to shared ownership using `Arc<RwLock<...>>` or a dedicated state store abstraction.

Why:

- fixes routing feedback integrity,
- avoids clone divergence,
- keeps selector behavior predictable.

### Recommendation 3: Separate model selection from auth selection

Current code leans toward auth selection first. For this product, the better order is:

1. determine required model capabilities,
2. filter viable models,
3. score candidate routes,
4. select the best auth for the chosen model,
5. keep fallback routes ready.

Why:

- the gateway's value is model-aware routing, not only credential balancing.

### Recommendation 4: Define a compatibility matrix

Add explicit compatibility data for each model/provider route:

- streaming,
- tools,
- vision,
- max context,
- structured output support,
- endpoint family compatibility,
- tenant or auth restrictions.

Why:

- this prevents "looks valid on paper" routes that fail at execution time.

### Recommendation 5: Make tracing decision-oriented

Record structured fields such as:

- requested model,
- inferred capability requirements,
- candidate count,
- rejected candidates and reasons,
- chosen route,
- fallback sequence,
- retry count,
- final upstream outcome.

Why:

- this is the minimum observability needed to improve routing instead of guessing.

### Recommendation 6: Treat loop detection as a control-plane feature

Loop detection should not be bolted into prompt inspection alone. It should combine:

- route attempt count,
- upstream recursion checks,
- repeated output fingerprints,
- repeated tool invocation patterns,
- repeated identical provider/model failures.

Why:

- most practical loops are control-flow issues, not only text-pattern issues.

## Proposed Auto Smart-Routing Algorithm

The main design goal should be:

The operator should not need to hand-tune routing weights for normal operation.

Instead of manual parameter-heavy routing, the gateway should learn route quality from traffic and only require a small number of business constraints:

- hard safety constraints,
- cost ceiling or tenant budget class,
- optional provider deny/allow rules,
- optional latency SLO target.

Everything else should be derived automatically.

### Core idea

Treat routing as a constrained online learning problem:

1. infer what the request needs,
2. build all feasible route candidates,
3. predict which route is best for this request context,
4. keep some exploration to discover better routes,
5. update the model after every request outcome.

This is better than a fixed weighted formula because:

- request types are different,
- model/provider quality shifts over time,
- latency and reliability are non-stationary,
- provider quotas and temporary degradation are dynamic,
- manual tuning does not scale across tenants, models, and time windows.

### Recommended algorithm family

Use a hybrid of:

- hard-rule filtering,
- contextual scoring,
- contextual bandit exploration,
- online feedback updates.

In practical terms:

- hard constraints decide what is allowed,
- a learned score estimates route quality,
- a bandit policy decides exploration vs exploitation,
- online metrics continuously update route quality.

### Routing data model

Define one candidate as a full route:

`route_candidate = (model, provider_endpoint, auth)`

Do not learn only at the auth level.

The gateway should score full route candidates because the real outcome depends on:

- model capability,
- provider implementation quality,
- credential quota headroom,
- latency profile,
- tenant and time context,
- request shape.

### Step 1: Build request context automatically

For every inbound request, derive a `RouteRequestContext`:

- `required_capabilities`
- `preferred_capabilities`
- `estimated_input_tokens`
- `estimated_output_tokens`
- `requires_streaming`
- `requires_tools`
- `requires_vision`
- `reasoning_intensity`
- `latency_sensitivity`
- `budget_sensitivity`
- `tenant_id`
- `time_bucket`
- `request_family`

This should be inferred from request content and shape, not manually supplied.

Examples:

- image input or multimodal content -> `requires_vision = true`
- tool definitions present -> `requires_tools = true`
- very large prompt -> large-context route required
- background async task -> higher cost sensitivity, lower latency sensitivity
- interactive chat -> higher latency sensitivity

### Step 2: Hard constraint filtering

Before scoring, remove impossible routes.

A route is feasible only if all are true:

- model supports required capabilities,
- model can fit estimated context,
- auth is currently available,
- auth has not exceeded hard quota limits,
- provider endpoint supports the request mode,
- tenant policy allows this provider/model,
- recursion / loop guard does not block the route.

This step should be deterministic and strict.

This is where policy rules belong.

### Step 3: Build route features

For each feasible route, calculate features from recent history.

Recommended feature groups:

Request-model fit features:

- capability fit score,
- context fit margin,
- estimated cost,
- expected output quality tier,
- structured-output compatibility,
- tool-call compatibility.

Runtime health features:

- recent success probability,
- recent timeout probability,
- recent 429 probability,
- recent 5xx probability,
- cooldown status,
- circuit-breaker state.

Performance features:

- p50 latency,
- p95 latency,
- streaming first-token latency,
- token throughput,
- recent queue depth,
- in-flight concurrency.

Quota features:

- requests-per-minute headroom,
- tokens-per-minute headroom,
- projected exhaustion time,
- reserve-capacity consumption.

Context features:

- hour-of-day,
- weekday/weekend,
- tenant traffic class,
- request family,
- retry attempt number.

### Step 4: Predict route utility

For each feasible route, estimate:

- `P_success`
- `P_fast_enough`
- `expected_latency`
- `expected_cost`
- `P_loop_risk`

Then derive a utility value:

```text
utility(route, request)
  = value_of_success
  - failure_cost * (1 - P_success)
  - latency_cost * normalized_expected_latency
  - monetary_cost * normalized_expected_cost
  - loop_penalty * P_loop_risk
```

But these terms should not be hand-tuned per route.

Instead:

- normalize all terms automatically from observed history,
- learn global or tenant-level coefficients online,
- keep only a few top-level business priors.

### Step 5: Use a contextual bandit for selection

This is the key part that removes manual routing weights.

Recommended policy:

- use Thompson Sampling or LinUCB on route utility.

Why contextual bandits fit this problem:

- each request is one decision,
- the gateway gets feedback quickly,
- the environment changes over time,
- we care about best-next-choice more than full long-horizon planning.

#### Option A: Thompson Sampling

Maintain per-route posterior estimates for:

- success,
- latency class,
- quota failure risk,
- loop risk.

Then sample route quality from those posteriors and choose the highest sampled route.

Benefits:

- simple,
- robust,
- naturally balances exploration and exploitation,
- handles uncertainty well when a route is new or recently changed.

#### Option B: LinUCB / contextual linear bandit

Learn route value from features:

- request features,
- route features,
- recent runtime signals.

Then select the candidate with the highest:

`predicted_reward + uncertainty_bonus`

Benefits:

- better generalization across similar routes,
- better cold-start handling,
- more adaptive to request type differences.

Recommendation:

Start with Thompson Sampling plus a simple linear utility model. It is easier to debug than a heavier RL system and gives most of the value early.

### Step 6: Online feedback update

After every request, update the route model using real outcomes:

- success/failure,
- latency,
- output tokens,
- tool success,
- user cancellation,
- timeout,
- 429,
- 5xx,
- retry needed,
- fallback needed,
- loop guard triggered.

Each completed request should produce one normalized reward:

```text
reward =
  +1.0 for success
  - latency_penalty
  - cost_penalty
  - retry_penalty
  - loop_penalty
  - hard_failure_penalty
```

This reward does not need to be user-configured in detail.

It can be bootstrapped with simple defaults and then calibrated from observed SLO violations and budget pressure.

### Step 7: Auto-calibration instead of manual thresholds

To avoid manual parameters, the system should learn baselines from rolling history.

Examples:

- define "slow" as above the rolling p75 or p90 for comparable request class,
- define "expensive" relative to cheapest feasible route in that class,
- define "unstable" from recent failure posterior, not a fixed magic number,
- define "high loop risk" from repeated route/output patterns.

This is the difference between:

- manual routing: fixed thresholds,
- adaptive routing: thresholds derived from current operating context.

### Step 8: Multi-level fallback algorithm

The router should not produce only one winner. It should produce an ordered route plan.

Recommended output:

```text
primary_route
fallback_route_1
fallback_route_2
...
```

Fallback ranking should optimize for diversity, not only second-best score.

Example rule:

- prefer a fallback on a different auth,
- then a different provider implementation,
- then a lower-cost but still capable model,
- then a lower-tier safe fallback.

This avoids correlated failures.

### Step 9: Loop and runaway control in the routing algorithm

Loop prevention should be part of route utility, not a separate afterthought.

Add explicit penalties or hard blocks for:

- same route repeated too many times in one request,
- same output fingerprint repeated across retries,
- same tool-call pattern repeated with no state change,
- gateway-upstream target resolving back to itself,
- retry chain exceeding attempt budget.

This can be modeled as:

`P_loop_risk`

and also used as a hard block once a threshold is crossed.

### Step 10: Cold-start strategy

New routes always have sparse data. Avoid hand-configuring them.

Use inherited priors from:

- provider family,
- model tier,
- model capability class,
- tenant class,
- endpoint type.

Example:

- a new Anthropic flagship vision route can inherit priors from other Anthropic flagship vision routes until enough route-local data exists.

This avoids the "new route never selected" problem.

## Practical Algorithm Proposal

If you want one concrete algorithm to implement first, use this:

### AutoRoute-v1

1. Infer request requirements from request payload.
2. Filter infeasible routes using hard constraints.
3. Build route features from live metrics, health, quota, and request context.
4. Predict route reward with a simple linear model:
   `predicted_reward = w * features`
5. Use Thompson Sampling on the success/failure and timeout risk terms.
6. Rank candidates by:
   `sampled_reward - expected_cost_penalty - loop_risk_penalty`
7. Return top 3 routes as execution plan.
8. Update the model online after the response finishes.

### Why this is the right v1

- simple enough to implement,
- far less manual than fixed weight tuning,
- explainable,
- supports online learning,
- naturally extends to time/quota adaptation,
- works with local SQLite persistence.

## What should still remain manual

A fully parameterless router is not realistic. A few things should remain explicit:

- hard safety rules,
- provider allow/deny rules,
- tenant budget classes,
- maximum retry depth,
- compliance restrictions,
- whether premium models are allowed for a tenant.

These are business constraints, not optimization parameters.

The router should learn performance behavior automatically, but business intent should stay explicit.

## What not to do

Avoid these designs:

- a giant fixed weighted formula with dozens of hand-tuned constants,
- a separate branching strategy for every mode,
- pure round-robin with policy patches,
- full RL before a contextual bandit baseline exists,
- manual per-model score tables.

These approaches either do not adapt enough or become too hard to reason about.

## Suggested Near-Term Deliverables

If the goal is to move this repo from foundation to usable localhost router quickly, the best next sequence is:

1. implement one real passthrough endpoint with upstream forwarding,
2. replace the placeholder router with a real routing pipeline,
3. fix shared-state feedback for health and metrics,
4. make capability-based model selection explicit,
5. emit route-decision traces,
6. add retry and recursion guards,
7. only then deepen time/quota adaptation.

This order follows KISS and YAGNI:

- first make the gateway actually route,
- then make routing correct,
- then make routing intelligent,
- then make routing adaptive.

## Final Assessment

This repository has the right ingredients, but they are not yet assembled into the product it describes.

The best interpretation of the current codebase is:

- `model-registry` is a strong metadata and policy foundation,
- `smart-routing` is a selection toolkit with an unfinished orchestration layer,
- `tracing` is a local middleware package, not yet an observability system,
- `crates/gateway` is a stub server that still needs real proxy behavior.

The highest-value improvement is not adding more strategy types. It is connecting request parsing, model filtering, route scoring, upstream forwarding, and feedback updates into one coherent routing pipeline.
