# Adding a New Provider

This guide explains how to add support for a new LLM provider (e.g., Mistral, Cohere).

## Step 1: Add Provider to the Enum

Edit `packages/model-registry/src/categories.rs`:

```rust
pub enum ProviderCategory {
    Anthropic,
    OpenAI,
    Google,
    Mistral,    // Add your provider
}
```

## Step 2: Implement Enum Methods

```rust
impl ProviderCategory {
    pub fn as_str(&self) -> &str {
        match self {
            ProviderCategory::Mistral => "mistral",
            // ... other variants
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "mistral" => Some(ProviderCategory::Mistral),
            // ... other variants
            _ => None,
        }
    }
}
```

## Step 3: Update Provider Detection

Update `get_provider_category()` in the `ModelCategorization` trait:

```rust
fn get_provider_category(&self) -> ProviderCategory {
    match self.provider.as_str() {
        "mistral" => ProviderCategory::Mistral,
        // ... other providers
        _ => ProviderCategory::Anthropic,
    }
}
```

## Step 4: Add Model Information

Add model data with pricing and capabilities:

```rust
ModelInfo {
    id: "mistral-large-2".to_string(),
    name: "Mistral Large 2".to_string(),
    provider: "mistral".to_string(),
    context_window: 128000,
    max_output_tokens: 4096,
    input_price_per_million: 3.0,
    output_price_per_million: 9.0,
    capabilities: ModelCapabilities {
        streaming: true,
        tools: true,
        vision: false,
        thinking: false,
    },
    rate_limits: RateLimits {
        requests_per_minute: 60,
        tokens_per_minute: 500000,
    },
    source: DataSource::Static,
}
```

## Step 5: Update Tier Classification (Optional)

Add flagship/fast models to `get_tier()`:

```rust
let flagship_models = HashSet::from([
    "mistral-large-2",
    // ... existing models
]);
```

## Step 6: Test

```bash
cargo test -p model-registry
```
