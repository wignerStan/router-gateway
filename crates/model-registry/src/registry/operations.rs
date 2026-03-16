use super::{
    broadcast, Arc, CachedModelInfo, CancellationToken, CapabilityCategory, ContextWindowCategory,
    CostCategory, FetchResult, HashMap, ModelCategorization, ModelInfo, Mutex, ProviderCategory,
    Registry, RegistryConfig, RwLock, TierCategory, Utc,
};

use tokio::task::JoinSet;

impl Registry {
    /// Creates a new model registry with default configuration.
    ///
    /// Uses a [`StaticFetcher`](crate::StaticFetcher) that returns no models.
    /// Replace the fetcher via [`with_config`](Self::with_config) for real data.
    ///
    /// # Examples
    ///
    /// ```
    /// # use model_registry::Registry;
    /// # #[tokio::main]
    /// # async fn main() {
    /// let registry = Registry::new();
    /// assert_eq!(registry.cached_count().await, 0);
    /// # }
    /// ```
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
                    },
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
    /// Returns a map of modelID -> `ModelInfo` for found models.
    pub async fn get_multiple(
        &self,
        model_ids: &[String],
    ) -> Result<HashMap<String, ModelInfo>, Box<dyn std::error::Error + Send + Sync>> {
        if model_ids.is_empty() {
            return Ok(HashMap::new());
        }

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
                set.spawn(async move { (id.clone(), registry.get(&id).await) });
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
    /// If `model_ids` is empty, refreshes all models from the fetcher.
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
    /// If `model_ids` is empty, clears the entire cache.
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
                    () = token.cancelled() => {
                        break;
                    }
                }
            }
        });

        self._background_handle = Some(handle);
    }
}
