# API Transformation (External Service)

> **Note:** API format conversion is handled by **CLIProxyAPI** as an external service.
> This gateway focuses on **smart routing** decisions, not format transformation.

## Architecture

```
┌─────────────┐     ┌──────────────────┐     ┌─────────────────┐
│   Client    │────▶│  Gateway         │────▶│  CLIProxyAPI    │
│  (OpenAI/   │     │  (Smart Routing) │     │  (Format        │
│   Anthropic)│     │                  │     │   Conversion)   │
└─────────────┘     └──────────────────┘     └─────────────────┘
                           │
                           ▼
                    ┌──────────────────┐
                    │ - Model Registry │
                    │ - Health Metrics │
                    │ - Weight Calc    │
                    │ - Auth Selection │
                    └──────────────────┘
```

## Gateway Responsibilities

This gateway handles:

1. **Model Registry** - Caching and serving model metadata
2. **Smart Routing** - Weighted credential/auth selection
3. **Health Tracking** - Monitoring upstream availability
4. **Metrics Collection** - Latency and error rate tracking

## CLIProxyAPI Responsibilities

The external CLIProxyAPI service handles:

1. **Format Detection** - Identifying OpenAI, Anthropic, or Gemini formats
2. **Request Transformation** - Converting between API formats
3. **Response Transformation** - Normalizing responses back to client format

### Format Pairs (CLIProxyAPI)

| Source | Target     | Direction     |
| ------ | ---------- | ------------- |
| OpenAI | Claude     | Bidirectional |
| OpenAI | Gemini     | Bidirectional |
| OpenAI | Gemini-CLI | Bidirectional |
| Claude | Gemini     | Bidirectional |

### Key Transformation Patterns (CLIProxyAPI)

For reference, these patterns are implemented in CLIProxyAPI:

#### OpenAI ↔ Claude

| OpenAI Field                  | Claude Field                           |
| ----------------------------- | -------------------------------------- |
| `messages[].content` (string) | `messages[].content` (array of blocks) |
| `messages[role=system]`       | `system` (root-level field)            |
| `tool_calls`                  | `tool_use` content blocks              |
| `reasoning_effort`            | `thinking.budget_tokens`               |

#### OpenAI ↔ Gemini

| OpenAI Field                | Gemini Field                   |
| --------------------------- | ------------------------------ |
| `messages[]`                | `contents[]`                   |
| `content`                   | `parts[]`                      |
| `messages[role=system]`     | `systemInstruction`            |
| `temperature`, `max_tokens` | `generationConfig.*`           |
| `reasoning_effort`          | `thinkingConfig.thinkingLevel` |
| `tool_calls`                | `functionCall`                 |

## Gateway API Endpoints

| Endpoint          | Purpose                    |
| ----------------- | -------------------------- |
| `GET /`           | Service info               |
| `GET /health`     | Health check               |
| `GET /api/models` | List available models      |
| `GET /api/route`  | Get routing recommendation |

## Configuration

Gateway routing is configured via `SmartRoutingConfig`:

```rust
struct SmartRoutingConfig {
    enabled: bool,
    strategy: String,      // "weighted", "time_aware", "quota_aware", "adaptive"
    weight: WeightConfig,  // Configurable weight factors
    health: HealthConfig,
}
```

See `packages/smart-routing/src/config.rs` for full configuration options.

---

_Format conversion details are documented in CLIProxyAPI.
This gateway delegates transformation to that external service._
