# Architecture Overview

This document explains the system architecture of the LLM Gateway, focusing on the design decisions and trade-offs that shape its behavior.

## High-Level Architecture

The gateway implements a proxy pattern with intelligent routing capabilities:

```
                                    +------------------+
                                    |   Model Registry |
                                    |  (Model Catalog) |
                                    +--------+---------+
                                             |
                                             v
+--------+     +------------+     +----------+----------+     +-------------+
| Client | --> |  Gateway   | --> |   Smart Routing     | --> | CLIProxyAPI |
|        |     |  (Axum)    |     |   (Weight-based)    |     | (External)  |
+--------+     +-----+------+     +----------+----------+     +------+------+
                     |                       |                       |
                     |                       v                       |
                     |              +--------+--------+              |
                     |              | Health Tracking |              |
                     |              +--------+--------+              |
                     |                       |                       |
                     |                       v                       v
                     |              +--------+--------+     +--------+--------+
                     +------------> | Metrics Store   | <---|  LLM Provider   |
                                    |    (SQLite)     |     |  (Anthropic/    |
                                    +-----------------+     |   OpenAI/Google)|
                                                            +-----------------+
```

## Core Packages

### model-registry

The model registry maintains a catalog of available LLM models with their metadata.

**Purpose:** Centralized model metadata management.

**Key Features:**

- 5-dimension classification system (Capability, Tier, Cost, Context, Provider)
- Async filtering and lookup
- External data source integration (ModelsDev, LiteLLM)

**Why a Registry?**
Model metadata is scattered across providers with inconsistent naming and capabilities. The registry normalizes this data, enabling intelligent routing decisions based on consistent criteria.

### smart-routing

The smart routing package implements credential selection with health awareness.

**Purpose:** Select the best credential for each request.

**Key Components:**

- **Weight Calculator:** Combines multiple factors into a selection score
- **Health Manager:** Tracks credential health with state machine
- **Metrics Store:** Persists request history in SQLite

**Why Weight-Based Selection?**
Round-robin and random selection ignore credential performance differences. Weight-based selection allows the system to prefer faster, more reliable credentials while still giving degraded credentials a chance to recover.

### llm-tracing

Request/response observability layer.

**Purpose:** Provide visibility into LLM interactions.

**Key Features:**

- Request tracing with correlation IDs
- Tower middleware integration
- Structured logging support

### core

Shared utilities and common types.

**Purpose:** Reduce code duplication across packages.

---

## Key Design Decisions

### Why SQLite for Persistence?

**Decision:** Use SQLite for metrics and health data storage.

**Rationale:**

- Local-first: No external database dependency
- Concurrent reads: Multiple async tasks can read simultaneously
- Simple deployment: Single file, easy backup/restore
- Sufficient scale: Designed for single-instance gateway (not distributed)

**Trade-offs:**

- Write serialization: All writes go through single connection
- No horizontal scaling: Cannot distribute across multiple gateway instances
- Acceptable for: Local development tools, single-tenant deployments

### Why CLIProxyAPI as External Service?

**Decision:** The CLIProxyAPI runs as a separate process, not embedded in the gateway.

**Rationale:**

- **Separation of concerns:** Gateway focuses on HTTP routing; CLIProxyAPI handles LLM protocol translation
- **Independent scaling:** Can scale proxy instances without changing gateway
- **Fault isolation:** Proxy crashes don't affect gateway stability
- **Language flexibility:** Could implement proxy in different language if needed

**Trade-offs:**

- **Network overhead:** Additional hop between gateway and proxy
- **Deployment complexity:** Two services to manage instead of one
- **Debugging:** Must trace across process boundaries

### Why 5-Dimension Classification?

**Decision:** Classify models across Capability, Tier, Cost, Context, and Provider dimensions.

**Rationale:**

- **Capability:** Filter models by features (vision, tools, thinking)
- **Tier:** Select quality/speed trade-off (flagship, standard, fast)
- **Cost:** Budget optimization (ultra-premium to economy)
- **Context:** Fit requests within context limits
- **Provider:** Route to specific vendor APIs

**Trade-offs:**

- **Complexity:** 5 dimensions require more configuration
- **Over-classification:** Some models may not fit neatly into categories
- **Benefits outweigh costs:** Enables fine-grained routing policies

---

## Health State Machine

Credentials transition through three health states based on request outcomes:

```
                    +----------+
                    | Healthy  |<-----------------+
                    +----+-----+                  |
                         |                        |
                    5xx error              3+ successes
                         |                        |
                         v                        |
                    +----------+                  |
                    | Degraded |<-----------+     |
                    +----+-----+            |     |
                         |                  |     |
              429 rate limit         success |     |
                         |                  |     |
                         v                  |     |
                    +------------+          |     |
                    | Unhealthy  |----------+     |
                    +-----+------+                |
                          |                       |
               cooldown expires                   |
                          |                       |
                          +-----------------------+
```

**State Transitions:**

- **Healthy -> Degraded:** Rate limit (429) or intermittent errors
- **Degraded -> Healthy:** Consecutive successes exceed threshold
- **Degraded -> Unhealthy:** Consecutive failures exceed threshold
- **Unhealthy -> Degraded:** Cooldown period expires

**Why This Design?**

- Prevents thundering herd: Unhealthy credentials get cooldown period
- Allows recovery: Degraded credentials still receive traffic (reduced)
- Hysteresis: State changes require multiple data points, preventing flapping

---

## Weight Calculation

Credential selection uses weighted random choice:

```
weight = success_rate_score * W1
       + latency_score * W2
       + health_score * W3
       + load_score * W4
       + priority_score * W5

Where:
- success_rate_score: 0-1 based on recent success rate
- latency_score: 1/(1 + latency_ms/1000) -- inverse relationship
- health_score: Healthy=1.0, Degraded=0.6, Unhealthy=0.1
- load_score: Considers quota, model availability, recent requests
- priority_score: User-assigned priority (-100 to 100, normalized)

Penalties multiply the final weight:
- Unhealthy: * 0.1
- Degraded: * 0.7
- Quota exceeded: * 0.01
- Unavailable: * 0.01
```

**Why This Formula?**

- **Multiplicative penalties:** A single severe issue (quota exceeded) can effectively remove a credential
- **Additive components:** Multiple small issues combine gradually
- **Inverse latency:** Lower latency yields higher score, but with diminishing returns

---

## Request Flow

1. **Client sends request** to Gateway (Axum on port 3000)
2. **Gateway extracts** model ID and request parameters
3. **Model Registry** validates model exists and returns metadata
4. **Smart Routing** selects credential based on:
   - Model availability for credential
   - Credential health status
   - Recent performance metrics
   - User-assigned priority
5. **Gateway forwards** request to CLIProxyAPI with selected credential
6. **Response flows back** through same path
7. **Metrics recorded** for future routing decisions

---

## Package Dependencies

```
                    +-------+
                    | core  |
                    +---+---+
                        ^
                        |
        +---------------+---------------+
        |               |               |
+-------+-------+ +-----+-----+ +-------+-------+
| model-registry | |   tracing | | smart-routing |
+---------------+ +-----------+ +-------+-------+
        ^                               ^
        |                               |
        +---------------+---------------+
                        |
                +-------+-------+
                |    gateway    |
                +---------------+
```

**Dependency Rules:**

- `core` has no internal dependencies (foundational)
- Packages depend on `core` for shared types
- `gateway` is the top-level aggregator
- No circular dependencies between packages
