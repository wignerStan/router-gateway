# Model Classification System

> **Note:** Model classification is implemented in `crates/model-registry/src/categories.rs`.
> This documentation reflects the actual Rust implementation.

## Overview

The gateway uses a multi-dimensional classification system for intelligent routing decisions. Each model is categorized across 5 dimensions to enable filtering and selection.

---

## Classification Dimensions

### 1. Capability Category

Defines functional capabilities supported by a model.

| Category    | Description                 | Example Models                          |
| ----------- | --------------------------- | --------------------------------------- |
| `Vision`    | Image/vision input support  | claude-sonnet-4, gpt-4o, gemini-2.5-pro |
| `Tools`     | Function calling support    | Most modern models                      |
| `Streaming` | Streaming response support  | All models                              |
| `Thinking`  | Extended thinking/reasoning | claude-sonnet-4, o1, gemini-2.5-flash   |

**Implementation:** `crates/model-registry/src/categories.rs:8`

```rust
pub enum CapabilityCategory {
    Vision,
    Tools,
    Streaming,
    Thinking,
}
```

### 2. Tier Category

Quality/performance tier for model selection.

| Tier       | Description                   | Criteria                           | Examples                                      |
| ---------- | ----------------------------- | ---------------------------------- | --------------------------------------------- |
| `Flagship` | Highest quality, highest cost | Price ≥$15/1M or known flagship ID | claude-opus-4, gpt-4-turbo, gemini-2.5-pro    |
| `Standard` | Balanced quality/cost         | Default                            | claude-sonnet-4, gpt-4o                       |
| `Fast`     | Lowest cost, highest speed    | Price ≤$1/1M or known fast ID      | claude-haiku-4, gpt-4o-mini, gemini-2.5-flash |

**Implementation:** `crates/model-registry/src/categories.rs:29`

```rust
pub enum TierCategory {
    Flagship,  // Opus, GPT-4
    Standard,  // Sonnet, GPT-4o
    Fast,      // Haiku, Flash
}
```

### 3. Cost Category

Pricing band based on input price per million tokens.

| Category       | Price Range | Examples                    |
| -------------- | ----------- | --------------------------- |
| `UltraPremium` | ≥$50/1M     | High-end enterprise models  |
| `Premium`      | $10-50/1M   | claude-opus-4               |
| `Standard`     | $1-10/1M    | claude-sonnet-4, gpt-4o     |
| `Economy`      | <$1/1M      | claude-haiku-4, gpt-4o-mini |

**Implementation:** `crates/model-registry/src/categories.rs:51`

```rust
pub enum CostCategory {
    UltraPremium,  // $50+/1M tokens
    Premium,       // $10-50/1M tokens
    Standard,      // $1-10/1M tokens
    Economy,       // <$1/1M tokens
}
```

### 4. Context Window Category

Context size band for request fitting.

| Category | Token Range | Examples                                   |
| -------- | ----------- | ------------------------------------------ |
| `Small`  | <32K        | Legacy models                              |
| `Medium` | 32K-128K    | gpt-4, claude-instant                      |
| `Large`  | 128K-500K   | claude-sonnet-4, gpt-4o, gemini-2.5-pro    |
| `Ultra`  | ≥500K       | gemini-2.5-flash (1M), gemini-1.5-pro (2M) |

**Implementation:** `crates/model-registry/src/categories.rs:76`

```rust
pub enum ContextWindowCategory {
    Small,   // <32K tokens
    Medium,  // 32K-128K tokens
    Large,   // 128K-500K tokens
    Ultra,   // 500K+ tokens
}
```

### 5. Provider Category

Model vendor for provider-specific routing.

**Implementation:** `crates/model-registry/src/categories.rs:102`

```rust
pub enum ProviderCategory {
    // Major Cloud Providers
    Anthropic,
    OpenAI,
    Google,

    // Emerging AI Companies
    XAI,        // xAI Grok
    DeepSeek,   // DeepSeek
    Mistral,    // Mistral AI
    Cohere,     // Cohere
    Perplexity, // Perplexity AI

    // Chinese Providers
    Alibaba,    // Qwen/Tongyi
    Zhipu,      // GLM/ChatGLM
    Baidu,      // ERNIE/Wenxin
    Moonshot,   // Kimi
    ByteDance,  // Doubao

    // Open Source / Community
    Meta,       // Llama
    MetaLlama,  // Alias for Meta
    Databricks, // Dolly
    Stability,  // Stable LM

    // Cloud Platforms
    Amazon,     // AWS Bedrock
    Azure,      // Azure OpenAI
    VertexAI,   // GCP Vertex AI

    // Other
    Other,
}
```

| Provider     | Display Name       | Aliases       |
| ------------ | ------------------ | ------------- |
| `Anthropic`  | Anthropic          | -             |
| `OpenAI`     | OpenAI             | -             |
| `Google`     | Google             | gemini        |
| `XAI`        | xAI                | x-ai, grok    |
| `DeepSeek`   | DeepSeek           | -             |
| `Mistral`    | Mistral AI         | mistralai     |
| `Cohere`     | Cohere             | -             |
| `Perplexity` | Perplexity         | -             |
| `Alibaba`    | Alibaba (Qwen)     | qwen, tongyi  |
| `Zhipu`      | Zhipu AI (GLM)     | glm, chatglm  |
| `Baidu`      | Baidu (ERNIE)      | ernie, wenxin |
| `Moonshot`   | Moonshot (Kimi)    | kimi          |
| `ByteDance`  | ByteDance (Doubao) | doubao        |
| `Meta`       | Meta               | -             |
| `MetaLlama`  | Meta (Llama)       | llama         |
| `Amazon`     | Amazon Bedrock     | bedrock, aws  |
| `Azure`      | Azure OpenAI       | -             |
| `VertexAI`   | Google Vertex AI   | vertex        |

---

## Modality Category (New)

Input/output modality types for multi-modal routing.

| Modality    | Description        |
| ----------- | ------------------ |
| `Text`      | Text input/output  |
| `Image`     | Image input        |
| `Audio`     | Audio input/output |
| `Video`     | Video input        |
| `Embedding` | Embedding output   |
| `Code`      | Code generation    |

---

## Routing Policy System (New)

The policy system enables fine-grained routing rules based on multi-dimensional classification.

### Policy Structure

```rust
pub struct RoutingPolicy {
    pub id: String,
    pub name: String,
    pub priority: i32,
    pub enabled: bool,
    pub filters: PolicyFilters,
    pub action: PolicyAction,
    pub conditions: Vec<PolicyCondition>,
}
```

### Policy Filters

Combine multiple dimensions for matching:

```rust
pub struct PolicyFilters {
    pub capabilities: Vec<CapabilityFilter>,  // require/prefer/exclude
    pub tiers: Vec<TierCategory>,              // any match
    pub costs: Vec<CostCategory>,              // any match
    pub context_windows: Vec<ContextWindowCategory>, // any match
    pub providers: Vec<ProviderCategory>,      // any match
    pub modalities: Vec<ModalityCategory>,     // all must match
}
```

### Policy Actions

| Action   | Description                      |
| -------- | -------------------------------- |
| `prefer` | Boost weight of matching models  |
| `avoid`  | Reduce weight of matching models |
| `block`  | Exclude matching models          |
| `weight` | Apply custom weight factor       |

### Policy Conditions

Time and context-based conditions:

| Condition Type | Description         | Example               |
| -------------- | ------------------- | --------------------- |
| `TimeOfDay`    | Hour of day (0-23)  | Off-peak routing      |
| `DayOfWeek`    | Day of week (0-6)   | Weekend policies      |
| `TokenCount`   | Request token count | Large context routing |
| `TenantId`     | User/tenant ID      | Per-tenant policies   |
| `ModelFamily`  | Model family name   | Family-specific rules |
| `Custom`       | Custom metadata     | Flexible extensions   |

### Example Policies

```rust
use model_registry::{RoutingPolicy, CapabilityCategory, TierCategory, ProviderCategory};

// Vision-required policy
let vision_policy = RoutingPolicy::new("vision_required", "Vision Required")
    .with_priority(30)
    .with_capability(CapabilityCategory::Vision, "require")
    .with_action("prefer");

// Cost optimization during off-peak
let off_peak = RoutingPolicy::new("off_peak_cost", "Off-Peak Cost Optimization")
    .with_priority(10)
    .with_tier(TierCategory::Fast)
    .with_action("weight")
    .with_weight_factor(1.5);

// Provider preference
let prefer_anthropic = RoutingPolicy::new("prefer_anthropic", "Prefer Anthropic")
    .with_priority(15)
    .with_provider(ProviderCategory::Anthropic)
    .with_action("prefer");
```

### Policy Templates

Pre-built templates for common use cases:

```rust
use model_registry::policy::templates;

let cost_opt = templates::cost_optimization();
let perf_first = templates::performance_first();
let quality_first = templates::quality_first();
let vision_req = templates::vision_required();
let thinking_req = templates::thinking_required();
let prefer_openai = templates::prefer_provider(ProviderCategory::OpenAI);
```

---

## ModelCategorization Trait

The trait provides categorization methods for any `ModelInfo`:

```rust
pub trait ModelCategorization {
    fn get_categories(&self) -> ModelCategorySet;
    fn get_capability_categories(&self) -> Vec<CapabilityCategory>;
    fn get_tier(&self) -> TierCategory;
    fn get_cost_category(&self) -> CostCategory;
    fn get_context_category(&self) -> ContextWindowCategory;
    fn get_provider_category(&self) -> ProviderCategory;

    // Filter methods
    fn has_all_capabilities(&self, caps: &[CapabilityCategory]) -> bool;
    fn has_any_capability(&self, caps: &[CapabilityCategory]) -> bool;
    fn is_in_tier(&self, tier: TierCategory) -> bool;
    fn is_in_cost_range(&self, cost: CostCategory) -> bool;
    fn is_in_context_range(&self, context: ContextWindowCategory) -> bool;
    fn is_from_provider(&self, provider: ProviderCategory) -> bool;
}
```

---

## Registry Filter Methods

The `Registry` provides async filter methods:

```rust
// Filter by single dimension
async fn filter_by_capability(cap: CapabilityCategory) -> Vec<ModelInfo>
async fn filter_by_tier(tier: TierCategory) -> Vec<ModelInfo>
async fn filter_by_cost(cost: CostCategory) -> Vec<ModelInfo>
async fn filter_by_context_window(context: ContextWindowCategory) -> Vec<ModelInfo>
async fn filter_by_provider(provider: ProviderCategory) -> Vec<ModelInfo>

// Find optimal model
async fn find_best_fit(tokens: usize) -> Option<ModelInfo>
```

**Implementation:** `crates/model-registry/src/registry/mod.rs:305-430`

---

## Tier Detection Logic

Tier is determined by both explicit model IDs and price thresholds:

### Flagship Models (by ID)

- `claude-opus-4`, `claude-opus-4-20250514`
- `gpt-4`, `gpt-4-turbo`, `gpt-4-0314`
- `gemini-2.5-pro`

### Fast Models (by ID)

- `claude-haiku-4`, `claude-haiku-4-20250514`
- `gpt-4o-mini`, `gpt-4o-mini-2024-07-18`
- `gemini-2.0-flash-exp`, `gemini-2.5-flash`, `gemini-2.5-flash-exp`
- `gemini-1.5-flash`, `gemini-1.5-flash-8b`, `gemini-1.5-flash-exp`

### Price-Based Fallback

- `input_price_per_million >= 15.0` → Flagship
- `input_price_per_million <= 1.0` → Fast
- Otherwise → Standard

---

## Usage Examples

### Filter models by capability

```rust
use model_registry::{Registry, CapabilityCategory};

let registry = Registry::new();
let vision_models = registry.filter_by_capability(CapabilityCategory::Vision).await;
```

### Find cheapest model for context

```rust
let best = registry.find_best_fit(100_000).await;
// Returns cheapest model that can fit 100K tokens
```

### Check model capabilities

```rust
let model = registry.get("claude-sonnet-4-20250514").await?;
if model.has_all_capabilities(&[CapabilityCategory::Vision, CapabilityCategory::Tools]) {
    // Model supports both vision and function calling
}
```

---

## ModelInfo Structure

Each model contains complete metadata:

```rust
pub struct ModelInfo {
    pub id: String,                      // "claude-sonnet-4-20250514"
    pub name: String,                    // "Claude Sonnet 4"
    pub provider: String,                // "anthropic"
    pub context_window: usize,           // 200000
    pub max_output_tokens: usize,        // 4096
    pub input_price_per_million: f64,    // 3.0
    pub output_price_per_million: f64,   // 15.0
    pub capabilities: ModelCapabilities, // {streaming, tools, vision, thinking}
    pub rate_limits: RateLimits,         // {requests_per_minute, tokens_per_minute}
    pub source: DataSource,              // Static, ModelsDev, LiteLLM, Local
}
```

---

## Summary

| Dimension  | Categories                             | Primary Use             |
| ---------- | -------------------------------------- | ----------------------- |
| Capability | 4 (Vision, Tools, Streaming, Thinking) | Feature filtering       |
| Tier       | 3 (Flagship, Standard, Fast)           | Quality/speed selection |
| Cost       | 4 (UltraPremium → Economy)             | Budget optimization     |
| Context    | 4 (Small → Ultra)                      | Request fitting         |
| Provider   | 20+ (see Provider Category above)      | Provider routing        |
