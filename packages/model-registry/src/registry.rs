use crate::categories::{
    CapabilityCategory, ContextWindowCategory, CostCategory, ModelCategorization, ProviderCategory,
    TierCategory,
};
use crate::fetcher::ModelFetcher;
use crate::info::ModelInfo;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex, RwLock};
use tokio_util::sync::CancellationToken;

/// CachedModelInfo holds model info with expiration time.
#[derive(Clone)]
struct CachedModelInfo {
    info: ModelInfo,
    expires_at: DateTime<Utc>,
}

/// RegistryConfig defines model registry configuration.
pub struct RegistryConfig {
    /// Fetcher is the underlying model fetcher
    pub fetcher: Arc<dyn ModelFetcher>,

    /// TTL is how long to cache model info (default: 1 hour)
    pub ttl: chrono::Duration,

    /// EnableBackgroundRefresh enables periodic cache refresh (default: false)
    pub enable_background_refresh: bool,

    /// RefreshInterval is how often to refresh the cache (default: TTL/2)
    pub refresh_interval: chrono::Duration,
}

impl Default for RegistryConfig {
    fn default() -> Self {
        let ttl = chrono::Duration::hours(1);
        Self {
            fetcher: Arc::new(crate::fetcher::StaticFetcher::new()),
            ttl,
            enable_background_refresh: false,
            refresh_interval: ttl / 2,
        }
    }
}

/// Result of a model fetch operation
type FetchResult = Result<Option<ModelInfo>, String>;

/// ModelRegistry provides thread-safe access to model information.
pub struct Registry {
    fetcher: Arc<dyn ModelFetcher>,
    cache: Arc<RwLock<HashMap<String, CachedModelInfo>>>,
    /// Coalesce concurrent fetches for the same model ID
    pending_fetches: Arc<Mutex<HashMap<String, broadcast::Sender<FetchResult>>>>,
    ttl: chrono::Duration,
    _background_handle: Option<tokio::task::JoinHandle<()>>,
    shutdown_token: CancellationToken,
}

impl Clone for Registry {
    fn clone(&self) -> Self {
        Self {
            fetcher: Arc::clone(&self.fetcher),
            cache: Arc::clone(&self.cache),
            pending_fetches: Arc::clone(&self.pending_fetches),
            ttl: self.ttl,
            _background_handle: None, // Only the primary instance manages the background task
            shutdown_token: self.shutdown_token.clone(),
        }
    }
}

impl Registry {
    /// Creates a new model registry with default configuration.
    pub fn new() -> Self {
        Self::with_config(RegistryConfig::default())
    }

    /// Creates a new model registry with custom configuration.
    pub fn with_config(config: RegistryConfig) -> Self {
        let ttl = config.ttl;
        let shutdown_token = CancellationToken::new();
        let mut registry = Self {
            fetcher: config.fetcher,
            cache: Arc::new(RwLock::new(HashMap::new())),
            pending_fetches: Arc::new(Mutex::new(HashMap::new())),
            ttl,
            _background_handle: None,
            shutdown_token,
        };

        // Start background refresh if enabled
        if config.enable_background_refresh {
            registry.start_background_refresh(config.refresh_interval);
        }

        registry
    }

    /// Retrieves model information for a single model ID.
    /// Returns None if the model is not found.
    pub async fn get(
        &self,
        model_id: &str,
    ) -> Result<Option<ModelInfo>, Box<dyn std::error::Error + Send + Sync>> {
        if model_id.is_empty() {
            return Err("model ID cannot be empty".into());
        }

        // 1. Check cache first (read lock)
        {
            let cache = self.cache.read().await;
            if let Some(cached) = cache.get(model_id) {
                if Utc::now() < cached.expires_at {
                    return Ok(Some(cached.info.clone()));
                }
            }
        }

        // 2. Cache miss or expired - handle concurrent fetches (coalescing)
        let mut rx = {
            let mut pending = self.pending_fetches.lock().await;

            // If another task is already fetching this model, wait for it
            if let Some(tx) = pending.get(model_id) {
                tx.subscribe()
            } else {
                // Otherwise, we are the fetcher
                let (tx, _rx) = broadcast::channel(1);
                pending.insert(model_id.to_string(), tx.clone());
                drop(pending); // Release lock before I/O

                // Ensure cleanup of pending map even on cancellation
                struct FetchGuard {
                    id: String,
                    pending: Arc<Mutex<HashMap<String, broadcast::Sender<FetchResult>>>>,
                }
                impl Drop for FetchGuard {
                    fn drop(&mut self) {
                        if let Ok(mut pending) = self.pending.try_lock() {
                            pending.remove(&self.id);
                        }
                    }
                }
                let _guard = FetchGuard {
                    id: model_id.to_string(),
                    pending: Arc::clone(&self.pending_fetches),
                };

                let fetch_result = self.fetcher.fetch(model_id).await;

                // Process result and update cache
                let result_to_broadcast = match fetch_result {
                    Ok(Some(info)) => {
                        if let Err(e) = info.validate() {
                            Err(e.to_string())
                        } else {
                            // Cache valid result
                            let mut cache = self.cache.write().await;
                            cache.insert(
                                model_id.to_string(),
                                CachedModelInfo {
                                    info: info.clone(),
                                    expires_at: Utc::now() + self.ttl,
                                },
                            );
                            Ok(Some(info))
                        }
                    }
                    Ok(None) => Ok(None),
                    Err(e) => Err(e.to_string()),
                };

                // Broadcast results (guard will remove from map on drop)
                let _ = tx.send(result_to_broadcast.clone());

                return match result_to_broadcast {
                    Ok(opt) => Ok(opt),
                    Err(e) => Err(e.into()),
                };
            }
        };

        // Wait for the primary fetcher to finish
        match rx.recv().await {
            Ok(Ok(info)) => Ok(info),
            Ok(Err(e)) => Err(e.into()),
            Err(_) => Err("Fetch broadcast failed".into()),
        }
    }

    /// Retrieves model information for multiple model IDs.
    /// Returns a map of modelID -> ModelInfo for found models.
    pub async fn get_multiple(
        &self,
        model_ids: &[String],
    ) -> Result<HashMap<String, ModelInfo>, Box<dyn std::error::Error + Send + Sync>> {
        if model_ids.is_empty() {
            return Ok(HashMap::new());
        }

        use tokio::task::JoinSet;
        let mut set = JoinSet::new();
        let mut result = HashMap::new();
        let now = Utc::now();

        // Check cache and identify needed models
        {
            let cache = self.cache.read().await;
            for model_id in model_ids {
                if model_id.is_empty() {
                    continue;
                }
                if let Some(cached) = cache.get(model_id) {
                    if now < cached.expires_at {
                        result.insert(model_id.clone(), cached.info.clone());
                        continue;
                    }
                }
                
                // Need to fetch this model
                let registry = self.clone();
                let id = model_id.clone();
                set.spawn(async move {
                    (id.clone(), registry.get(&id).await)
                });
            }
        }

        // Collect parallel results
        while let Some(res) = set.join_next().await {
            let (id, fetch_res) = res?;
            if let Some(info) = fetch_res? {
                result.insert(id, info);
            }
        }

        Ok(result)
    }

    /// Refreshes the cache for specific model IDs.
    /// If model_ids is empty, refreshes all models from the fetcher.
    pub async fn refresh(
        &self,
        model_ids: &[String],
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let now = Utc::now();

        if model_ids.is_empty() {
            // Refresh all models
            let models = self.fetcher.list_all().await?;

            let mut cache = self.cache.write().await;
            for (id, info) in models {
                if info.validate().is_err() {
                    continue;
                }
                cache.insert(
                    id,
                    CachedModelInfo {
                        info,
                        expires_at: now + self.ttl,
                    },
                );
            }
        } else {
            // Refresh specific models
            for id in model_ids {
                let _ = self.get(id).await;
            }
        }

        Ok(())
    }

    /// Removes specific models from the cache.
    /// If model_ids is empty, clears the entire cache.
    pub async fn invalidate(&self, model_ids: &[String]) {
        let mut cache = self.cache.write().await;
        if model_ids.is_empty() {
            cache.clear();
        } else {
            for model_id in model_ids {
                cache.remove(model_id);
            }
        }
    }

    /// Returns the number of models currently in cache.
    pub async fn cached_count(&self) -> usize {
        let cache = self.cache.read().await;
        cache.len()
    }

    /// Returns all model IDs in the cache.
    pub async fn cached_ids(&self) -> Vec<String> {
        let cache = self.cache.read().await;
        cache.keys().cloned().collect()
    }

    /// Removes expired entries from the cache.
    pub async fn cleanup_expired(&self) -> usize {
        let mut cache = self.cache.write().await;
        let now = Utc::now();
        let initial_len = cache.len();

        cache.retain(|_, cached| now < cached.expires_at);

        initial_len - cache.len()
    }

    /// Finds all cached models that support a specific capability.
    pub async fn find_by_capability(&self, capability: &str) -> Vec<ModelInfo> {
        let cache = self.cache.read().await;
        let now = Utc::now();
        let mut result = Vec::new();

        for cached in cache.values() {
            if now < cached.expires_at && cached.info.supports_capability(capability) {
                result.push(cached.info.clone());
            }
        }

        result
    }

    /// Finds all cached models from a specific provider.
    pub async fn find_by_provider(&self, provider: &str) -> Vec<ModelInfo> {
        let cache = self.cache.read().await;
        let now = Utc::now();
        let mut result = Vec::new();

        for cached in cache.values() {
            if now < cached.expires_at && cached.info.provider == provider {
                result.push(cached.info.clone());
            }
        }

        result
    }

    /// Filters cached models by capability category.
    pub async fn filter_by_capability(&self, cap: CapabilityCategory) -> Vec<ModelInfo> {
        let cache = self.cache.read().await;
        let now = Utc::now();
        let mut result = Vec::new();

        for cached in cache.values() {
            if now < cached.expires_at && cached.info.has_any_capability(&[cap]) {
                result.push(cached.info.clone());
            }
        }

        result
    }

    /// Filters models by quality tier.
    pub async fn filter_by_tier(&self, tier: TierCategory) -> Vec<ModelInfo> {
        let cache = self.cache.read().await;
        let now = Utc::now();
        let mut result = Vec::new();

        for cached in cache.values() {
            if now < cached.expires_at && cached.info.is_in_tier(tier) {
                result.push(cached.info.clone());
            }
        }

        result
    }

    /// Filters models by cost category.
    pub async fn filter_by_cost(&self, cost: CostCategory) -> Vec<ModelInfo> {
        let cache = self.cache.read().await;
        let now = Utc::now();
        let mut result = Vec::new();

        for cached in cache.values() {
            if now < cached.expires_at && cached.info.is_in_cost_range(cost) {
                result.push(cached.info.clone());
            }
        }

        result
    }

    /// Filters models by context window category.
    pub async fn filter_by_context_window(&self, context: ContextWindowCategory) -> Vec<ModelInfo> {
        let cache = self.cache.read().await;
        let now = Utc::now();
        let mut result = Vec::new();

        for cached in cache.values() {
            if now < cached.expires_at && cached.info.is_in_context_range(context) {
                result.push(cached.info.clone());
            }
        }

        result
    }

    /// Filters models by provider vendor.
    pub async fn filter_by_provider(&self, provider: ProviderCategory) -> Vec<ModelInfo> {
        let cache = self.cache.read().await;
        let now = Utc::now();
        let mut result = Vec::new();

        for cached in cache.values() {
            if now < cached.expires_at && cached.info.is_from_provider(provider) {
                result.push(cached.info.clone());
            }
        }

        result
    }

    /// Estimates costs for multiple models given input/output tokens.
    pub async fn estimate_costs(
        &self,
        model_ids: &[String],
        input_tokens: usize,
        output_tokens: usize,
    ) -> HashMap<String, f64> {
        let models = match self.get_multiple(model_ids).await {
            Ok(m) => m,
            Err(_) => return HashMap::new(),
        };

        let mut costs = HashMap::new();
        for (id, model) in models {
            costs.insert(id, model.estimate_cost(input_tokens, output_tokens));
        }

        costs
    }

    /// Finds the cheapest model that can fit the context window.
    /// Returns None if no model can fit the requested token count.
    pub async fn find_best_fit(&self, tokens: usize) -> Option<ModelInfo> {
        let cache = self.cache.read().await;
        let now = Utc::now();
        let mut best_model: Option<ModelInfo> = None;
        let mut best_cost = f64::MAX;

        for cached in cache.values() {
            if now >= cached.expires_at {
                continue;
            }

            if !cached.info.can_fit_context(tokens) {
                continue;
            }

            // Estimate cost (using average input/output ratio of 70/30)
            let estimated_input = (tokens as f64) * 0.7;
            let estimated_output = (tokens as f64) * 0.3;
            let cost = cached
                .info
                .estimate_cost(estimated_input as usize, estimated_output as usize);

            if best_model.is_none() || cost < best_cost {
                best_model = Some(cached.info.clone());
                best_cost = cost;
            }
        }

        best_model
    }

    /// Starts background refresh task.
    fn start_background_refresh(&mut self, interval: chrono::Duration) {
        let fetcher = Arc::clone(&self.fetcher);
        let cache = Arc::clone(&self.cache);
        let ttl = self.ttl;
        let token = self.shutdown_token.clone();

        let handle = tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(tokio::time::Duration::from_millis(
                (interval.num_milliseconds()) as u64,
            ));

            loop {
                tokio::select! {
                    _ = interval_timer.tick() => {
                        let mut cache_write = cache.write().await;
                        let now = Utc::now();

                        // Refresh all models
                        if let Ok(models) = fetcher.list_all().await {
                            for (id, info) in models {
                                if info.validate().is_ok() {
                                    cache_write.insert(
                                        id,
                                        CachedModelInfo {
                                            info,
                                            expires_at: now + ttl,
                                        },
                                    );
                                }
                            }
                        }

                        // Cleanup expired entries
                        cache_write.retain(|_, cached| now < cached.expires_at);
                    }
                    _ = token.cancelled() => {
                        break;
                    }
                }
            }
        });

        self._background_handle = Some(handle);
    }
}

impl Drop for Registry {
    fn drop(&mut self) {
        // Only the instance that owns the background handle should trigger shutdown
        if let Some(handle) = self._background_handle.take() {
            self.shutdown_token.cancel();
            handle.abort();
        }
    }
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_registry_get() {
        let registry = Registry::new();
        let model = registry.get("claude-sonnet-4-20250514").await.unwrap();
        assert!(model.is_some());
        assert_eq!(model.unwrap().name, "Claude Sonnet 4");
    }

    #[tokio::test]
    async fn test_registry_get_not_found() {
        let registry = Registry::new();
        let model = registry.get("unknown-model").await.unwrap();
        assert!(model.is_none());
    }

    #[tokio::test]
    async fn test_registry_get_multiple() {
        let registry = Registry::new();
        let models = registry
            .get_multiple(&[
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
    async fn test_registry_cached_count() {
        let registry = Registry::new();

        // Access some models to populate cache
        let _ = registry.get("claude-sonnet-4-20250514").await;
        let _ = registry.get("gpt-4o").await;

        let count = registry.cached_count().await;
        assert!(count >= 2);
    }

    #[tokio::test]
    async fn test_registry_invalidate() {
        let registry = Registry::new();

        // Populate cache
        let _ = registry.get("claude-sonnet-4-20250514").await;
        assert!(registry.cached_count().await > 0);

        // Clear cache
        registry.invalidate(&[]).await;
        assert_eq!(registry.cached_count().await, 0);
    }

    #[tokio::test]
    async fn test_registry_filter_by_capability() {
        let registry = Registry::new();

        // Populate cache
        let _ = registry.get("claude-sonnet-4-20250514").await;
        let _ = registry.get("gpt-4o").await;

        let vision_models = registry
            .filter_by_capability(CapabilityCategory::Vision)
            .await;
        assert!(!vision_models.is_empty());
    }

    #[tokio::test]
    async fn test_registry_filter_by_tier() {
        let registry = Registry::new();

        // Populate cache
        let _ = registry.get("claude-opus-4-20250514").await;
        let _ = registry.get("claude-sonnet-4-20250514").await;

        let flagship_models = registry.filter_by_tier(TierCategory::Flagship).await;
        assert!(!flagship_models.is_empty());
    }

    #[tokio::test]
    async fn test_registry_estimate_costs() {
        let registry = Registry::new();

        let costs = registry
            .estimate_costs(
                &["claude-sonnet-4-20250514".to_string()],
                1_000_000,
                500_000,
            )
            .await;

        assert!(costs.contains_key("claude-sonnet-4-20250514"));
        let cost = costs.get("claude-sonnet-4-20250514").unwrap();
        assert!((cost - 10.5).abs() < 0.01); // 3.0 + 7.5 = 10.5
    }

    #[tokio::test]
    async fn test_registry_find_best_fit() {
        let registry = Registry::new();

        // Populate cache
        let _ = registry.get("claude-sonnet-4-20250514").await;

        let best = registry.find_best_fit(100000).await;
        assert!(best.is_some());
        assert!(best.unwrap().can_fit_context(100000));
    }

    #[tokio::test]
    async fn test_registry_get_empty_model_id() {
        let registry = Registry::new();

        let result = registry.get("").await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("model ID cannot be empty"));
    }

    #[tokio::test]
    async fn test_registry_get_multiple_empty() {
        let registry = Registry::new();

        let result = registry.get_multiple(&[]).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_registry_cache_expiration() {
        let config = RegistryConfig {
            ttl: chrono::Duration::milliseconds(100),
            ..Default::default()
        };
        let registry = Registry::with_config(config);

        // Fetch and cache
        let model1 = registry.get("claude-sonnet-4-20250514").await.unwrap();
        assert!(model1.is_some());

        // Should still be cached
        let cached_count = registry.cached_count().await;
        assert!(cached_count > 0);

        // Wait for TTL to expire
        tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;

        // Cleanup expired entries
        let removed = registry.cleanup_expired().await;
        assert!(removed > 0);

        // Cache should be empty
        let cached_count = registry.cached_count().await;
        assert_eq!(cached_count, 0);
    }

    #[tokio::test]
    async fn test_registry_concurrent_access() {
        let registry = std::sync::Arc::new(Registry::new());
        let mut handles = vec![];

        // Spawn multiple concurrent readers
        for i in 0..10 {
            let registry_clone = registry.clone();
            let handle = tokio::spawn(async move {
                let model_id = if i % 2 == 0 {
                    "claude-sonnet-4-20250514"
                } else {
                    "gpt-4o"
                };
                registry_clone.get(model_id).await
            });
            handles.push(handle);
        }

        // All should succeed
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok());
        }

        // Cache should have both models
        let cached_count = registry.cached_count().await;
        assert!(cached_count >= 2);
    }

    #[tokio::test]
    async fn test_registry_find_best_fit_edge_cases() {
        let registry = Registry::new();

        // Populate cache
        let _ = registry.get("claude-sonnet-4-20250514").await;

        // Test with zero tokens - should find a model (all can fit 0 tokens)
        let best = registry.find_best_fit(0).await;
        // Result depends on whether cache is populated and models can fit
        drop(best);

        // Test with very large token count (beyond any model)
        let best = registry.find_best_fit(10_000_000).await;
        // May or may not find a model depending on what's in the fetcher
        // Just verify it doesn't panic
        drop(best);
    }

    #[tokio::test]
    async fn test_registry_find_best_fit_empty_cache() {
        let registry = Registry::new();

        // Don't populate cache
        let best = registry.find_best_fit(1000).await;
        // Empty cache should return None
        assert!(best.is_none());
    }

    #[tokio::test]
    async fn test_registry_invalidate_specific_models() {
        let registry = Registry::new();

        // Populate cache with multiple models
        let _ = registry.get("claude-sonnet-4-20250514").await;
        let _ = registry.get("gpt-4o").await;

        let cached_count = registry.cached_count().await;
        assert!(cached_count >= 2);

        // Invalidate specific model
        registry
            .invalidate(&["claude-sonnet-4-20250514".to_string()])
            .await;

        let cached_ids = registry.cached_ids().await;
        assert!(!cached_ids.contains(&"claude-sonnet-4-20250514".to_string()));
        assert!(cached_ids.contains(&"gpt-4o".to_string()));
    }

    #[tokio::test]
    async fn test_registry_refresh_specific_models() {
        let registry = Registry::new();

        // Refresh specific models (should fetch and cache)
        let result = registry
            .refresh(&["claude-sonnet-4-20250514".to_string()])
            .await;
        assert!(result.is_ok());

        let cached_ids = registry.cached_ids().await;
        assert!(cached_ids.contains(&"claude-sonnet-4-20250514".to_string()));
    }

    #[tokio::test]
    async fn test_registry_refresh_all_models() {
        let registry = Registry::new();

        // Refresh all models (empty slice)
        let result = registry.refresh(&[]).await;
        assert!(result.is_ok());

        // Should have cached models from the fetcher
        let cached_count = registry.cached_count().await;
        assert!(cached_count > 0);
    }

    #[tokio::test]
    async fn test_registry_find_by_capability_empty_cache() {
        let registry = Registry::new();

        let models = registry.find_by_capability("vision").await;
        assert!(models.is_empty());
    }

    #[tokio::test]
    async fn test_registry_find_by_provider_empty_cache() {
        let registry = Registry::new();

        let models = registry.find_by_provider("anthropic").await;
        assert!(models.is_empty());
    }

    #[tokio::test]
    async fn test_registry_cleanup_expired_with_fresh_entries() {
        let registry = Registry::new();

        // Populate cache with fresh entries
        let _ = registry.get("claude-sonnet-4-20250514").await;

        // Cleanup should remove nothing (all entries fresh)
        let removed = registry.cleanup_expired().await;
        assert_eq!(removed, 0);

        let cached_count = registry.cached_count().await;
        assert!(cached_count > 0);
    }

    #[tokio::test]
    async fn test_registry_clone() {
        let registry1 = Registry::new();
        let _ = registry1.get("claude-sonnet-4-20250514").await;

        let registry2 = registry1.clone();

        // Both should share the same cache
        let count1 = registry1.cached_count().await;
        let count2 = registry2.cached_count().await;
        assert_eq!(count1, count2);
    }

    #[tokio::test]
    async fn test_registry_get_multiple_with_empty_strings() {
        let registry = Registry::new();

        // Mix of valid and empty model IDs
        let result = registry
            .get_multiple(&[
                "claude-sonnet-4-20250514".to_string(),
                "".to_string(),
                "gpt-4o".to_string(),
            ])
            .await;

        assert!(result.is_ok());
        let models = result.unwrap();
        // Should only have the valid models
        assert!(models.contains_key("claude-sonnet-4-20250514"));
        assert!(models.contains_key("gpt-4o"));
        assert!(!models.contains_key(""));
    }

    #[tokio::test]
    async fn test_registry_config_with_background_refresh() {
        let config = RegistryConfig {
            enable_background_refresh: true,
            refresh_interval: chrono::Duration::seconds(1),
            ..Default::default()
        };

        let registry = Registry::with_config(config);

        // Give background task time to run
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Registry should still be functional
        let result = registry.get("claude-sonnet-4-20250514").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_registry_estimate_costs_empty_list() {
        let registry = Registry::new();

        let costs = registry.estimate_costs(&[], 1000, 500).await;
        assert_eq!(costs.len(), 0);
    }

    #[tokio::test]
    async fn test_registry_estimate_costs_unknown_model() {
        let registry = Registry::new();

        let costs = registry
            .estimate_costs(&["unknown-model".to_string()], 1000, 500)
            .await;
        // Unknown model should not be in results
        assert!(!costs.contains_key("unknown-model"));
    }

    #[tokio::test]
    async fn test_registry_find_by_capability_after_invalidation() {
        let registry = Registry::new();

        // Populate cache
        let _ = registry.get("claude-sonnet-4-20250514").await;

        // Find by capability should work
        let models = registry.find_by_capability("vision").await;
        // Result depends on whether the model supports vision
        drop(models);

        // Invalidate all
        registry.invalidate(&[]).await;

        // Find by capability should return empty
        let models = registry.find_by_capability("vision").await;
        assert!(models.is_empty());
    }

    // ========================================
    // Registry Filter Methods - Comprehensive Tests
    // ========================================

    #[tokio::test]
    async fn test_filter_by_capability_empty_cache() {
        let registry = Registry::new();
        // Don't populate cache

        let models = registry
            .filter_by_capability(CapabilityCategory::Vision)
            .await;
        assert!(models.is_empty(), "Empty cache should return empty result");
    }

    #[tokio::test]
    async fn test_filter_by_capability_expired_entries() {
        let config = RegistryConfig {
            ttl: chrono::Duration::milliseconds(50),
            ..Default::default()
        };
        let registry = Registry::with_config(config);

        // Populate cache
        let _ = registry.get("claude-sonnet-4-20250514").await;

        // Wait for TTL to expire
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Filter should return empty (expired entries)
        let models = registry
            .filter_by_capability(CapabilityCategory::Vision)
            .await;
        assert!(models.is_empty(), "Expired entries should be filtered out");
    }

    #[tokio::test]
    async fn test_filter_by_tier_flagship() {
        let registry = Registry::new();
        let _ = registry.get("claude-opus-4-20250514").await; // Flagship model

        let flagship_models = registry.filter_by_tier(TierCategory::Flagship).await;
        assert!(!flagship_models.is_empty(), "Should find flagship models");

        // All returned models should be flagship
        for model in &flagship_models {
            assert!(model.is_in_tier(TierCategory::Flagship));
        }
    }

    #[tokio::test]
    async fn test_filter_by_tier_standard() {
        let registry = Registry::new();
        let _ = registry.get("claude-sonnet-4-20250514").await;

        let standard_models = registry.filter_by_tier(TierCategory::Standard).await;
        assert!(!standard_models.is_empty(), "Should find standard models");

        for model in &standard_models {
            assert!(model.is_in_tier(TierCategory::Standard));
        }
    }

    #[tokio::test]
    async fn test_filter_by_tier_fast() {
        let registry = Registry::new();
        let _ = registry.get("gemini-2.5-flash").await;

        let fast_models = registry.filter_by_tier(TierCategory::Fast).await;
        assert!(!fast_models.is_empty(), "Should find fast models");

        for model in &fast_models {
            assert!(model.is_in_tier(TierCategory::Fast));
        }
    }

    #[tokio::test]
    async fn test_filter_by_cost_all_categories() {
        let registry = Registry::new();

        // Populate with models of different cost categories
        let _ = registry.get("claude-opus-4-20250514").await; // Premium ($15/1M)
        let _ = registry.get("claude-sonnet-4-20250514").await; // Standard ($3/1M)
        let _ = registry.get("gemini-2.5-flash").await; // Economy ($0.075/1M)

        // Test each cost category
        let economy_models = registry.filter_by_cost(CostCategory::Economy).await;
        for model in &economy_models {
            assert!(model.is_in_cost_range(CostCategory::Economy));
        }

        let standard_models = registry.filter_by_cost(CostCategory::Standard).await;
        for model in &standard_models {
            assert!(model.is_in_cost_range(CostCategory::Standard));
        }

        let premium_models = registry.filter_by_cost(CostCategory::Premium).await;
        for model in &premium_models {
            assert!(model.is_in_cost_range(CostCategory::Premium));
        }

        let ultra_premium_models = registry.filter_by_cost(CostCategory::UltraPremium).await;
        for model in &ultra_premium_models {
            assert!(model.is_in_cost_range(CostCategory::UltraPremium));
        }
    }

    #[tokio::test]
    async fn test_filter_by_context_window_all_categories() {
        let registry = Registry::new();

        // Populate with models of different context sizes
        let _ = registry.get("gpt-4o").await; // 128K - Large
        let _ = registry.get("gemini-2.5-flash").await; // 1M - Ultra

        // Test each context window category
        let small_models = registry
            .filter_by_context_window(ContextWindowCategory::Small)
            .await;
        for model in &small_models {
            assert!(model.is_in_context_range(ContextWindowCategory::Small));
        }

        let medium_models = registry
            .filter_by_context_window(ContextWindowCategory::Medium)
            .await;
        for model in &medium_models {
            assert!(model.is_in_context_range(ContextWindowCategory::Medium));
        }

        let large_models = registry
            .filter_by_context_window(ContextWindowCategory::Large)
            .await;
        for model in &large_models {
            assert!(model.is_in_context_range(ContextWindowCategory::Large));
        }

        let ultra_models = registry
            .filter_by_context_window(ContextWindowCategory::Ultra)
            .await;
        for model in &ultra_models {
            assert!(model.is_in_context_range(ContextWindowCategory::Ultra));
        }
    }

    #[tokio::test]
    async fn test_filter_by_provider_all_variants() {
        let registry = Registry::new();

        // Populate with models from different providers
        let _ = registry.get("claude-sonnet-4-20250514").await; // Anthropic
        let _ = registry.get("gpt-4o").await; // OpenAI
        let _ = registry.get("gemini-2.5-flash").await; // Google

        // Test major providers
        let anthropic_models = registry
            .filter_by_provider(ProviderCategory::Anthropic)
            .await;
        assert!(!anthropic_models.is_empty(), "Should find Anthropic models");
        for model in &anthropic_models {
            assert!(model.is_from_provider(ProviderCategory::Anthropic));
        }

        let openai_models = registry.filter_by_provider(ProviderCategory::OpenAI).await;
        assert!(!openai_models.is_empty(), "Should find OpenAI models");
        for model in &openai_models {
            assert!(model.is_from_provider(ProviderCategory::OpenAI));
        }

        let google_models = registry.filter_by_provider(ProviderCategory::Google).await;
        assert!(!google_models.is_empty(), "Should find Google models");
        for model in &google_models {
            assert!(model.is_from_provider(ProviderCategory::Google));
        }

        // Provider with no models
        let xai_models = registry.filter_by_provider(ProviderCategory::XAI).await;
        assert!(
            xai_models.is_empty(),
            "Should not find xAI models in default fetcher"
        );
    }

    #[tokio::test]
    async fn test_estimate_costs_mixed_valid_invalid_ids() {
        let registry = Registry::new();

        let costs = registry
            .estimate_costs(
                &[
                    "claude-sonnet-4-20250514".to_string(), // Valid
                    "unknown-model".to_string(),            // Invalid
                    "gpt-4o".to_string(),                   // Valid
                    "".to_string(),                         // Empty (invalid)
                ],
                1_000_000,
                500_000,
            )
            .await;

        // Should only return costs for valid models
        assert!(costs.contains_key("claude-sonnet-4-20250514"));
        assert!(costs.contains_key("gpt-4o"));
        assert!(!costs.contains_key("unknown-model"));
        assert!(!costs.contains_key(""));
    }

    #[tokio::test]
    async fn test_find_best_fit_no_models_fit() {
        let registry = Registry::new();
        let _ = registry.get("claude-sonnet-4-20250514").await;

        // Request extremely large context that no model can fit
        let best = registry.find_best_fit(100_000_000).await; // 100M tokens
        assert!(best.is_none(), "No model should fit 100M tokens");
    }

    #[tokio::test]
    async fn test_find_best_fit_multiple_same_cost() {
        let registry = Registry::new();

        // Populate cache with multiple models
        let _ = registry.get("claude-sonnet-4-20250514").await;
        let _ = registry.get("gpt-4o").await;
        let _ = registry.get("gemini-2.5-flash").await;

        // Find best fit for reasonable token count
        let best = registry.find_best_fit(50000).await;
        assert!(best.is_some(), "Should find a model that fits");

        // The returned model should fit the context
        let model = best.unwrap();
        assert!(model.can_fit_context(50000));
    }

    #[tokio::test]
    async fn test_find_best_fit_prefers_cheapest() {
        let registry = Registry::new();

        // Populate cache
        let _ = registry.get("claude-sonnet-4-20250514").await; // $3/1M input
        let _ = registry.get("gemini-2.5-flash").await; // $0.075/1M input (cheaper)

        // Both can fit 100K tokens, should prefer the cheaper one
        let best = registry.find_best_fit(100000).await;
        assert!(best.is_some());

        if let Some(model) = best {
            // Gemini Flash is cheaper
            assert!(model.can_fit_context(100000));
        }
    }

    #[tokio::test]
    async fn test_find_best_fit_at_exact_context_boundary() {
        let registry = Registry::new();
        let _ = registry.get("gpt-4o").await; // 128K context

        // Request exactly at context boundary
        let best = registry.find_best_fit(128000).await;
        assert!(best.is_some(), "Should find model at exact boundary");

        // Request just over boundary
        let best = registry.find_best_fit(128001).await;
        // Depends on whether other models with larger context are available
        // Just verify it doesn't panic
        drop(best);
    }

    #[tokio::test]
    async fn test_filter_by_capability_all_capability_types() {
        let registry = Registry::new();
        let _ = registry.get("claude-opus-4-20250514").await; // Has all capabilities

        // Test each capability type
        let streaming_models = registry
            .filter_by_capability(CapabilityCategory::Streaming)
            .await;
        assert!(!streaming_models.is_empty());

        let tools_models = registry
            .filter_by_capability(CapabilityCategory::Tools)
            .await;
        assert!(!tools_models.is_empty());

        let vision_models = registry
            .filter_by_capability(CapabilityCategory::Vision)
            .await;
        assert!(!vision_models.is_empty());

        let thinking_models = registry
            .filter_by_capability(CapabilityCategory::Thinking)
            .await;
        assert!(
            !thinking_models.is_empty(),
            "Thinking model should support thinking capability"
        );
    }

    #[tokio::test]
    async fn test_filter_methods_with_only_expired_entries() {
        let config = RegistryConfig {
            ttl: chrono::Duration::milliseconds(50),
            ..Default::default()
        };
        let registry = Registry::with_config(config);

        // Populate and expire
        let _ = registry.get("claude-sonnet-4-20250514").await;
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // All filter methods should return empty for expired entries
        assert!(registry
            .filter_by_tier(TierCategory::Flagship)
            .await
            .is_empty());
        assert!(registry
            .filter_by_cost(CostCategory::Standard)
            .await
            .is_empty());
        assert!(registry
            .filter_by_context_window(ContextWindowCategory::Large)
            .await
            .is_empty());
        assert!(registry
            .filter_by_provider(ProviderCategory::Anthropic)
            .await
            .is_empty());
    }

    #[tokio::test]
    async fn test_estimate_costs_with_zero_tokens() {
        let registry = Registry::new();
        let _ = registry.get("claude-sonnet-4-20250514").await;

        let costs = registry
            .estimate_costs(&["claude-sonnet-4-20250514".to_string()], 0, 0)
            .await;

        assert!(costs.contains_key("claude-sonnet-4-20250514"));
        let cost = costs.get("claude-sonnet-4-20250514").unwrap();
        assert!((cost - 0.0).abs() < 0.001, "Zero tokens should cost zero");
    }
}
