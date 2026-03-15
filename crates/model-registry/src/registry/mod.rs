//! Thread-safe model registry with caching, coalesced fetches, and background refresh.
//!
//! The registry wraps a [`ModelFetcher`] and caches results with configurable TTL.
//! Concurrent fetches for the same model ID are coalesced into a single request.

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

mod operations;

#[cfg(test)]
mod tests;
