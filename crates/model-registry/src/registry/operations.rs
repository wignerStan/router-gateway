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
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(RegistryConfig::default())
    }

    /// Creates a new model registry with custom configuration.
    #[must_use]
    pub fn with_config(config: RegistryConfig) -> Self {
        let ttl = config.ttl;
        let shutdown_token = CancellationToken::new();
        let mut registry = Self {
            fetcher: config.fetcher,
            cache: Arc::new(RwLock::new(HashMap::new())),
            pending_fetches: Arc::new(Mutex::new(HashMap::new())),
            ttl,
            background_handle: None,
            shutdown_token,
        };

        // Start background refresh if enabled
        if config.enable_background_refresh {
            registry.start_background_refresh(config.refresh_interval);
        }

        registry
    }

    /// Retrieves model information for a single model ID.
    ///
    /// Returns `None` if the model is not found.
    ///
    /// # Errors
    ///
    /// Returns an error if the model ID is empty or the underlying fetcher fails.
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

                let (tx, _rx) = broadcast::channel(1);
                pending.insert(model_id.to_string(), tx.clone());
                drop(pending); // Release lock before I/O

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
                            {
                                let mut cache = self.cache.write().await;
                                cache.insert(
                                    model_id.to_string(),
                                    CachedModelInfo {
                                        info: info.clone(),
                                        expires_at: Utc::now() + self.ttl,
                                    },
                                );
                            }
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

    /// Retrieves model information for multiple model IDs in parallel.
    ///
    /// Returns a map of model ID to [`ModelInfo`] for found models.
    ///
    /// # Errors
    ///
    /// Returns an error if any underlying fetch operation fails.
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
        let to_fetch: Vec<(String, Self)> = {
            let cache = self.cache.read().await;
            model_ids
                .iter()
                .filter(|model_id| !model_id.is_empty())
                .filter_map(|model_id| {
                    if let Some(cached) = cache.get(model_id) {
                        if now < cached.expires_at {
                            result.insert(model_id.clone(), cached.info.clone());
                            return None;
                        }
                    }
                    Some((model_id.clone(), self.clone()))
                })
                .collect()
        };

        for (id, registry) in to_fetch {
            set.spawn(async move { (id.clone(), registry.get(&id).await) });
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
    ///
    /// If `model_ids` is empty, refreshes all models from the fetcher.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying fetcher fails.
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
    ///
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

    /// Returns the number of non-expired models currently in the cache.
    #[must_use]
    pub async fn cached_count(&self) -> usize {
        let cache = self.cache.read().await;
        cache.len()
    }

    /// Returns all model IDs currently in the cache (may include expired entries).
    #[must_use]
    pub async fn cached_ids(&self) -> Vec<String> {
        let cache = self.cache.read().await;
        cache.keys().cloned().collect()
    }

    /// Removes expired entries from the cache and returns the count removed.
    #[must_use]
    pub async fn cleanup_expired(&self) -> usize {
        let mut cache = self.cache.write().await;
        let now = Utc::now();
        let initial_len = cache.len();

        cache.retain(|_, cached| now < cached.expires_at);

        initial_len - cache.len()
    }

    /// Finds all non-expired cached models that support a given capability.
    #[must_use]
    pub async fn find_by_capability(&self, capability: &str) -> Vec<ModelInfo> {
        let cache = self.cache.read().await;
        let now = Utc::now();
        cache
            .values()
            .filter(|cached| now < cached.expires_at && cached.info.supports_capability(capability))
            .map(|cached| cached.info.clone())
            .collect()
    }

    /// Finds all non-expired cached models from a given provider.
    #[must_use]
    pub async fn find_by_provider(&self, provider: &str) -> Vec<ModelInfo> {
        let cache = self.cache.read().await;
        let now = Utc::now();
        cache
            .values()
            .filter(|cached| now < cached.expires_at && cached.info.provider == provider)
            .map(|cached| cached.info.clone())
            .collect()
    }

    /// Filters non-expired cached models by capability category.
    #[must_use]
    pub async fn filter_by_capability(&self, cap: CapabilityCategory) -> Vec<ModelInfo> {
        let cache = self.cache.read().await;
        let now = Utc::now();
        cache
            .values()
            .filter(|cached| now < cached.expires_at && cached.info.has_any_capability(&[cap]))
            .map(|cached| cached.info.clone())
            .collect()
    }

    /// Filters non-expired cached models by quality tier.
    #[must_use]
    pub async fn filter_by_tier(&self, tier: TierCategory) -> Vec<ModelInfo> {
        let cache = self.cache.read().await;
        let now = Utc::now();
        cache
            .values()
            .filter(|cached| now < cached.expires_at && cached.info.is_in_tier(tier))
            .map(|cached| cached.info.clone())
            .collect()
    }

    /// Filters non-expired cached models by cost category.
    #[must_use]
    pub async fn filter_by_cost(&self, cost: CostCategory) -> Vec<ModelInfo> {
        let cache = self.cache.read().await;
        let now = Utc::now();
        cache
            .values()
            .filter(|cached| now < cached.expires_at && cached.info.is_in_cost_range(cost))
            .map(|cached| cached.info.clone())
            .collect()
    }

    /// Filters non-expired cached models by context window category.
    #[must_use]
    pub async fn filter_by_context_window(&self, context: ContextWindowCategory) -> Vec<ModelInfo> {
        let cache = self.cache.read().await;
        let now = Utc::now();
        cache
            .values()
            .filter(|cached| now < cached.expires_at && cached.info.is_in_context_range(context))
            .map(|cached| cached.info.clone())
            .collect()
    }

    /// Filters non-expired cached models by provider category.
    #[must_use]
    pub async fn filter_by_provider(&self, provider: ProviderCategory) -> Vec<ModelInfo> {
        let cache = self.cache.read().await;
        let now = Utc::now();
        cache
            .values()
            .filter(|cached| now < cached.expires_at && cached.info.is_from_provider(provider))
            .map(|cached| cached.info.clone())
            .collect()
    }

    /// Estimates costs for multiple models given input/output token counts.
    ///
    /// Returns a map of model ID to estimated cost in USD.
    /// Models that fail to fetch are silently omitted.
    #[must_use]
    pub async fn estimate_costs(
        &self,
        model_ids: &[String],
        input_tokens: usize,
        output_tokens: usize,
    ) -> HashMap<String, f64> {
        let Ok(models) = self.get_multiple(model_ids).await else {
            return HashMap::new();
        };

        let mut costs = HashMap::new();
        for (id, model) in models {
            costs.insert(id, model.estimate_cost(input_tokens, output_tokens));
        }

        costs
    }

    /// Finds the cheapest non-expired cached model that fits the context window.
    ///
    /// Returns `None` if no model can fit the requested token count.
    #[must_use]
    pub async fn find_best_fit(&self, tokens: usize) -> Option<ModelInfo> {
        let cache = self.cache.read().await;
        let now = Utc::now();
        cache
            .values()
            .filter(|cached| now < cached.expires_at && cached.info.can_fit_context(tokens))
            .min_by(|a, b| {
                let cost_a = a.info.estimate_cost(
                    (tokens as f64 * 0.7) as usize,
                    (tokens as f64 * 0.3) as usize,
                );
                let cost_b = b.info.estimate_cost(
                    (tokens as f64 * 0.7) as usize,
                    (tokens as f64 * 0.3) as usize,
                );
                cost_a
                    .partial_cmp(&cost_b)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|cached| cached.info.clone())
    }

    /// Starts a background task that periodically refreshes the cache.
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

        self.background_handle = Some(handle);
    }
}
