use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{PoisonError, RwLock};

use crate::info::{DataSource, ModelCapabilities, ModelInfo, RateLimits};

/// ModelFetcher defines the interface for fetching model information.
#[async_trait]
pub trait ModelFetcher: Send + Sync {
    /// Fetch retrieves model information for the given model ID.
    /// Returns `None` if the model is not found.
    ///
    /// # Errors
    /// Returns an error if the fetch operation fails.
    async fn fetch(
        &self,
        model_id: &str,
    ) -> Result<Option<ModelInfo>, Box<dyn std::error::Error + Send + Sync>>;

    /// FetchMultiple retrieves model information for multiple model IDs.
    /// Returns a map of model ID to `ModelInfo` for found models.
    ///
    /// # Errors
    /// Returns an error if the fetch operation fails.
    async fn fetch_multiple(
        &self,
        model_ids: &[String],
    ) -> Result<HashMap<String, ModelInfo>, Box<dyn std::error::Error + Send + Sync>>;

    /// ListAll returns all available models from this fetcher.
    ///
    /// # Errors
    /// Returns an error if the fetch operation fails.
    async fn list_all(
        &self,
    ) -> Result<HashMap<String, ModelInfo>, Box<dyn std::error::Error + Send + Sync>>;
}

/// Converts a `PoisonError` into a boxed error suitable for the trait return type.
fn lock_err(e: PoisonError<impl std::fmt::Debug>) -> Box<dyn std::error::Error + Send + Sync> {
    Box::new(std::io::Error::new(
        std::io::ErrorKind::Other,
        format!("lock poisoned: {e}"),
    ))
}

/// StaticFetcher provides hardcoded model data for common models.
pub struct StaticFetcher {
    models: RwLock<HashMap<String, ModelInfo>>,
}

impl StaticFetcher {
    /// Creates a new `StaticFetcher` with hardcoded model data.
    #[must_use]
    pub fn new() -> Self {
        Self {
            models: RwLock::new(Self::build_models()),
        }
    }

    /// Builds the static model registry without requiring a write lock.
    fn build_models() -> HashMap<String, ModelInfo> {
        let mut models = HashMap::new();

        // Claude Sonnet 4
        models.insert(
            "claude-sonnet-4-20250514".to_string(),
            ModelInfo {
                id: "claude-sonnet-4-20250514".to_string(),
                name: "Claude Sonnet 4".to_string(),
                provider: "anthropic".to_string(),
                context_window: 200_000,
                max_output_tokens: 8192,
                input_price_per_million: 3.0,
                output_price_per_million: 15.0,
                capabilities: ModelCapabilities {
                    streaming: true,
                    tools: true,
                    vision: true,
                    thinking: false,
                },
                rate_limits: RateLimits {
                    requests_per_minute: 60,
                    tokens_per_minute: 300_000,
                },
                source: DataSource::Static,
            },
        );

        // Claude Opus 4
        models.insert(
            "claude-opus-4-20250514".to_string(),
            ModelInfo {
                id: "claude-opus-4-20250514".to_string(),
                name: "Claude Opus 4".to_string(),
                provider: "anthropic".to_string(),
                context_window: 200_000,
                max_output_tokens: 8192,
                input_price_per_million: 15.0,
                output_price_per_million: 75.0,
                capabilities: ModelCapabilities {
                    streaming: true,
                    tools: true,
                    vision: true,
                    thinking: true,
                },
                rate_limits: RateLimits {
                    requests_per_minute: 60,
                    tokens_per_minute: 300_000,
                },
                source: DataSource::Static,
            },
        );

        // GPT-4o
        models.insert(
            "gpt-4o".to_string(),
            ModelInfo {
                id: "gpt-4o".to_string(),
                name: "GPT-4o".to_string(),
                provider: "openai".to_string(),
                context_window: 128_000,
                max_output_tokens: 4096,
                input_price_per_million: 2.50,
                output_price_per_million: 10.0,
                capabilities: ModelCapabilities {
                    streaming: true,
                    tools: true,
                    vision: true,
                    thinking: false,
                },
                rate_limits: RateLimits {
                    requests_per_minute: 500,
                    tokens_per_minute: 150_000,
                },
                source: DataSource::Static,
            },
        );

        // GPT-4-turbo
        models.insert(
            "gpt-4-turbo".to_string(),
            ModelInfo {
                id: "gpt-4-turbo".to_string(),
                name: "GPT-4 Turbo".to_string(),
                provider: "openai".to_string(),
                context_window: 128_000,
                max_output_tokens: 4096,
                input_price_per_million: 0.50,
                output_price_per_million: 2.0,
                capabilities: ModelCapabilities {
                    streaming: true,
                    tools: true,
                    vision: true,
                    thinking: false,
                },
                rate_limits: RateLimits {
                    requests_per_minute: 500,
                    tokens_per_minute: 300_000,
                },
                source: DataSource::Static,
            },
        );

        // Gemini 2.5 Pro
        models.insert(
            "gemini-2.5-pro".to_string(),
            ModelInfo {
                id: "gemini-2.5-pro".to_string(),
                name: "Gemini 2.5 Pro".to_string(),
                provider: "google".to_string(),
                context_window: 1_000_000,
                max_output_tokens: 8192,
                input_price_per_million: 1.25,
                output_price_per_million: 5.0,
                capabilities: ModelCapabilities {
                    streaming: true,
                    tools: true,
                    vision: true,
                    thinking: false,
                },
                rate_limits: RateLimits {
                    requests_per_minute: 60,
                    tokens_per_minute: 120_000,
                },
                source: DataSource::Static,
            },
        );

        // Gemini 2.5 Flash
        models.insert(
            "gemini-2.5-flash".to_string(),
            ModelInfo {
                id: "gemini-2.5-flash".to_string(),
                name: "Gemini 2.5 Flash".to_string(),
                provider: "google".to_string(),
                context_window: 1_000_000,
                max_output_tokens: 8192,
                input_price_per_million: 0.075,
                output_price_per_million: 0.30,
                capabilities: ModelCapabilities {
                    streaming: true,
                    tools: true,
                    vision: true,
                    thinking: false,
                },
                rate_limits: RateLimits {
                    requests_per_minute: 100,
                    tokens_per_minute: 1_000_000,
                },
                source: DataSource::Static,
            },
        );

        models
    }
}

impl Default for StaticFetcher {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ModelFetcher for StaticFetcher {
    async fn fetch(
        &self,
        model_id: &str,
    ) -> Result<Option<ModelInfo>, Box<dyn std::error::Error + Send + Sync>> {
        let models = self.models.read().map_err(lock_err)?;
        Ok(models.get(model_id).cloned())
    }

    async fn fetch_multiple(
        &self,
        model_ids: &[String],
    ) -> Result<HashMap<String, ModelInfo>, Box<dyn std::error::Error + Send + Sync>> {
        let models = self.models.read().map_err(lock_err)?;
        let mut result = HashMap::new();
        for model_id in model_ids {
            if let Some(model) = models.get(model_id) {
                result.insert(model_id.clone(), model.clone());
            }
        }
        Ok(result)
    }

    async fn list_all(
        &self,
    ) -> Result<HashMap<String, ModelInfo>, Box<dyn std::error::Error + Send + Sync>> {
        let models = self.models.read().map_err(lock_err)?;
        Ok(models.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_static_fetcher_fetch() {
        let fetcher = StaticFetcher::new();
        let model = fetcher.fetch("claude-sonnet-4-20250514").await.unwrap();
        assert!(model.is_some());
        assert_eq!(model.unwrap().name, "Claude Sonnet 4");
    }

    #[tokio::test]
    async fn test_static_fetcher_fetch_not_found() {
        let fetcher = StaticFetcher::new();
        let model = fetcher.fetch("unknown-model").await.unwrap();
        assert!(model.is_none());
    }

    #[tokio::test]
    async fn test_static_fetcher_fetch_multiple() {
        let fetcher = StaticFetcher::new();
        let models = fetcher
            .fetch_multiple(&[
                "claude-sonnet-4-20250514".to_string(),
                "gpt-4o".to_string(),
                "unknown".to_string(),
            ])
            .await
            .unwrap();

        assert_eq!(models.len(), 2);
        assert!(models.contains_key("claude-sonnet-4-20250514"));
        assert!(models.contains_key("gpt-4o"));
    }

    #[tokio::test]
    async fn test_static_fetcher_list_all() {
        let fetcher = StaticFetcher::new();
        let models = fetcher.list_all().await.unwrap();
        assert!(models.len() >= 6); // At least the 6 models we initialized
    }
    use std::panic::{self, AssertUnwindSafe};

    fn poison_lock(fetcher: &StaticFetcher) {
        let _ = panic::catch_unwind(AssertUnwindSafe(|| {
            let _guard = fetcher.models.write().unwrap();
            panic!("intentional poison for testing");
        }));
    }

    #[tokio::test]
    async fn test_fetch_returns_error_on_poisoned_lock() {
        let fetcher = StaticFetcher::new();
        poison_lock(&fetcher);
        let result = fetcher.fetch("test").await;
        assert!(
            result.is_err(),
            "fetch should return Err on poisoned lock, got {:?}",
            result
        );
    }

    #[tokio::test]
    async fn test_fetch_multiple_returns_error_on_poisoned_lock() {
        let fetcher = StaticFetcher::new();
        poison_lock(&fetcher);
        let result = fetcher.fetch_multiple(&["test".to_string()]).await;
        assert!(
            result.is_err(),
            "fetch_multiple should return Err on poisoned lock, got {:?}",
            result
        );
    }

    #[tokio::test]
    async fn test_list_all_returns_error_on_poisoned_lock() {
        let fetcher = StaticFetcher::new();
        poison_lock(&fetcher);
        let result = fetcher.list_all().await;
        assert!(
            result.is_err(),
            "list_all should return Err on poisoned lock, got {:?}",
            result
        );
    }
}
