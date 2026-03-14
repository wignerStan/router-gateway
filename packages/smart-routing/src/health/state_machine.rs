use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;
use std::sync::Arc;

use super::{AuthHealth, HealthManager, HealthStatus};

impl HealthManager {
    /// Create a new health manager.
    ///
    /// New credentials default to [`HealthStatus::Healthy`] until
    /// failures are recorded via [`update_from_result`](Self::update_from_result).
    ///
    /// # Examples
    ///
    /// ```
    /// # use smart_routing::{HealthManager, HealthConfig, HealthStatus};
    /// # #[tokio::main]
    /// # async fn main() {
    /// let config = HealthConfig::default();
    /// let manager = HealthManager::new(config);
    ///
    /// // Unknown credentials default to Healthy
    /// assert_eq!(manager.get_status("new-cred").await, HealthStatus::Healthy);
    /// # }
    /// ```
    pub fn new(config: crate::config::HealthConfig) -> Self {
        Self {
            health: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            config,
            max_entries: 10_000,
            cleanup_interval: 100,
            op_count: Arc::new(std::sync::atomic::AtomicI64::new(0)),
        }
    }

    /// Create a health manager with a limit
    pub fn with_limit(config: crate::config::HealthConfig, max_entries: usize) -> Self {
        Self {
            health: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            config,
            max_entries: if max_entries > 0 { max_entries } else { 10_000 },
            cleanup_interval: 100,
            op_count: Arc::new(std::sync::atomic::AtomicI64::new(0)),
        }
    }

    /// Get auth health status
    pub async fn get_status(&self, auth_id: &str) -> HealthStatus {
        if auth_id.is_empty() {
            return HealthStatus::Unhealthy;
        }

        let health = self.health.read().await;
        health
            .get(auth_id)
            .map_or(HealthStatus::Healthy, |h| h.status)
    }

    /// Get auth health details
    pub async fn get_health(&self, auth_id: &str) -> Option<AuthHealth> {
        if auth_id.is_empty() {
            return None;
        }

        let health = self.health.read().await;
        health.get(auth_id).cloned()
    }

    /// Update health status from execution result
    #[allow(clippy::significant_drop_tightening)]
    pub async fn update_from_result(&self, auth_id: &str, success: bool, status_code: i32) {
        if auth_id.is_empty() {
            return;
        }

        // Increment operation counter and check if cleanup is needed
        let op_count = self
            .op_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if op_count % self.cleanup_interval == 0 {
            let mut health = self.health.write().await;
            if health.len() > self.max_entries {
                self.cleanup_old_entries(&mut health).await;
            }
        }

        {
            let mut health = self.health.write().await;
            let entry = health
                .entry(auth_id.to_string())
                .or_insert_with(|| AuthHealth {
                    status: HealthStatus::Healthy,
                    consecutive_successes: 0,
                    consecutive_failures: 0,
                    last_status_change: Utc::now(),
                    last_check_time: Utc::now(),
                    unavailable_until: None,
                    error_counts: HashMap::new(),
                });

            let now = Utc::now();
            entry.last_check_time = now;

            // Record error type
            if status_code > 0 {
                *entry.error_counts.entry(status_code).or_insert(0) += 1;
            }

            // Update consecutive success/failure counts
            if success {
                entry.consecutive_successes += 1;
                entry.consecutive_failures = 0;
            } else {
                entry.consecutive_failures += 1;
                entry.consecutive_successes = 0;
            }

            // Calculate health status based on status code and consecutive counts
            let new_status = self.calculate_health_status(entry, success, status_code);

            // Update status if changed
            if new_status != entry.status {
                entry.status = new_status;
                entry.last_status_change = now;
            }

            // Set cooldown period for unhealthy credentials
            if entry.status == HealthStatus::Unhealthy
                && entry.consecutive_failures >= self.config.unhealthy_threshold
            {
                let cooldown = Duration::seconds(self.config.cooldown_period_seconds);
                entry.unavailable_until = Some(now + cooldown);
            }
        }
    }

    /// Calculate health status
    fn calculate_health_status(
        &self,
        health: &AuthHealth,
        _success: bool,
        status_code: i32,
    ) -> HealthStatus {
        // Check if status code is in unhealthy list
        if self.config.status_codes.unhealthy.contains(&status_code) {
            return HealthStatus::Unhealthy;
        }

        // Check if status code is in degraded list
        if self.config.status_codes.degraded.contains(&status_code) {
            return HealthStatus::Degraded;
        }

        // Check consecutive failure count
        if health.consecutive_failures >= self.config.unhealthy_threshold {
            return HealthStatus::Unhealthy;
        }

        // Recover based on consecutive success count
        if health.consecutive_successes >= self.config.healthy_threshold {
            return HealthStatus::Healthy;
        }

        // Maintain current status
        health.status
    }

    /// Check if auth is healthy
    pub async fn is_healthy(&self, auth_id: &str) -> bool {
        let status = self.get_status(auth_id).await;
        matches!(status, HealthStatus::Healthy | HealthStatus::Degraded)
    }

    /// Check if auth is available (healthy and not in cooldown)
    pub async fn is_available(&self, auth_id: &str) -> bool {
        if auth_id.is_empty() {
            return false;
        }

        let health = self.health.read().await;
        match health.get(auth_id) {
            Some(h) => {
                // Check if in cooldown period
                if let Some(unavailable_until) = h.unavailable_until {
                    if Utc::now() < unavailable_until {
                        return false;
                    }
                }
                // Check health status
                h.status != HealthStatus::Unhealthy
            },
            None => true, // No record means available by default
        }
    }

    /// Mark auth as unavailable
    #[allow(clippy::significant_drop_tightening)]
    pub async fn mark_unavailable(&self, auth_id: &str, duration: Duration) {
        if auth_id.is_empty() || duration.num_seconds() <= 0 {
            return;
        }

        {
            let mut health = self.health.write().await;
            let entry = health
                .entry(auth_id.to_string())
                .or_insert_with(|| AuthHealth {
                    status: HealthStatus::Healthy,
                    consecutive_successes: 0,
                    consecutive_failures: 0,
                    last_status_change: Utc::now(),
                    last_check_time: Utc::now(),
                    unavailable_until: None,
                    error_counts: HashMap::new(),
                });

            entry.unavailable_until = Some(Utc::now() + duration);
            entry.status = HealthStatus::Unhealthy;
            entry.last_status_change = Utc::now();
        }
    }

    /// Reset auth health status
    pub async fn reset(&self, auth_id: &str) {
        if auth_id.is_empty() {
            return;
        }

        let mut health = self.health.write().await;
        health.insert(
            auth_id.to_string(),
            AuthHealth {
                status: HealthStatus::Healthy,
                consecutive_successes: 0,
                consecutive_failures: 0,
                last_status_change: Utc::now(),
                last_check_time: Utc::now(),
                unavailable_until: None,
                error_counts: HashMap::new(),
            },
        );
    }

    /// Get count of healthy auths
    pub async fn get_healthy_count(&self, auth_ids: &[String]) -> i32 {
        let mut count = 0;
        for id in auth_ids {
            if self.is_healthy(id).await {
                count += 1;
            }
        }
        count
    }

    /// Get count of available auths
    pub async fn get_available_count(&self, auth_ids: &[String]) -> i32 {
        let mut count = 0;
        for id in auth_ids {
            if self.is_available(id).await {
                count += 1;
            }
        }
        count
    }

    /// Cleanup old entries to control memory growth
    async fn cleanup_old_entries(&self, health: &mut HashMap<String, AuthHealth>) {
        if self.max_entries == 0 {
            return;
        }

        // Collect entries with last check time
        let mut entries: Vec<(String, DateTime<Utc>)> = health
            .iter()
            .map(|(id, h)| (id.clone(), h.last_check_time))
            .collect();

        // Sort by last check time (oldest first)
        entries.sort_by(|a, b| a.1.cmp(&b.1));

        // Remove oldest entries if over limit
        let remove_count = entries.len().saturating_sub(self.max_entries);
        for (id, _) in entries.into_iter().take(remove_count) {
            health.remove(&id);
        }
    }

    /// Update the health configuration dynamically
    pub fn set_config(&mut self, config: crate::config::HealthConfig) {
        self.config = config;
    }
}
