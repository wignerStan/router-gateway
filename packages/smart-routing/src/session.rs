use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Session affinity management
///
/// Maintains provider affinity across multi-turn conversations.
/// Prefers the same provider for session continuation to ensure
/// consistent behavior and context awareness.
/// Session affinity record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionAffinity {
    /// Session identifier
    pub session_id: String,
    /// Preferred provider for this session
    pub preferred_provider: String,
    /// Last access timestamp
    pub last_access: DateTime<Utc>,
    /// Request count for this session
    pub request_count: u64,
}

/// Session affinity manager
pub struct SessionAffinityManager {
    /// Session affinity storage
    sessions: Arc<RwLock<HashMap<String, SessionAffinity>>>,
    /// Maximum sessions to store
    max_sessions: usize,
    /// Session TTL in seconds (default: 24 hours)
    session_ttl_seconds: i64,
}

impl Clone for SessionAffinityManager {
    fn clone(&self) -> Self {
        Self {
            sessions: Arc::clone(&self.sessions),
            max_sessions: self.max_sessions,
            session_ttl_seconds: self.session_ttl_seconds,
        }
    }
}

impl Default for SessionAffinityManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionAffinityManager {
    /// Create a new session affinity manager
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            max_sessions: 10_000,
            session_ttl_seconds: 24 * 60 * 60, // 24 hours
        }
    }

    /// Create a session affinity manager with custom limits
    pub fn with_limits(max_sessions: usize, session_ttl_seconds: i64) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            max_sessions: if max_sessions > 0 {
                max_sessions
            } else {
                10_000
            },
            session_ttl_seconds: if session_ttl_seconds > 0 {
                session_ttl_seconds
            } else {
                24 * 60 * 60
            },
        }
    }

    /// Get the preferred provider for a session
    pub async fn get_preferred_provider(&self, session_id: &str) -> Option<String> {
        if session_id.is_empty() {
            return None;
        }

        let sessions = self.sessions.read().await;
        sessions
            .get(session_id)
            .map(|affinity| affinity.preferred_provider.clone())
    }

    /// Set or update the preferred provider for a session
    pub async fn set_provider(&self, session_id: String, provider: String) -> Result<(), String> {
        if session_id.is_empty() {
            return Err("Session ID cannot be empty".to_string());
        }
        if provider.is_empty() {
            return Err("Provider cannot be empty".to_string());
        }

        let mut sessions = self.sessions.write().await;

        // Check if session exists
        let affinity = sessions
            .entry(session_id.clone())
            .or_insert_with(|| SessionAffinity {
                session_id: session_id.clone(),
                preferred_provider: provider.clone(),
                last_access: Utc::now(),
                request_count: 0,
            });

        // Update existing session
        affinity.preferred_provider = provider;
        affinity.last_access = Utc::now();
        affinity.request_count += 1;

        // Cleanup if over limit
        if sessions.len() > self.max_sessions {
            self.cleanup_expired(&mut sessions);
        }

        Ok(())
    }

    /// Check if a session has an affinity record
    pub async fn has_affinity(&self, session_id: &str) -> bool {
        if session_id.is_empty() {
            return false;
        }

        let sessions = self.sessions.read().await;
        sessions.contains_key(session_id)
    }

    /// Get session affinity details
    pub async fn get_affinity(&self, session_id: &str) -> Option<SessionAffinity> {
        if session_id.is_empty() {
            return None;
        }

        let sessions = self.sessions.read().await;
        sessions.get(session_id).cloned()
    }

    /// Remove a session affinity record
    pub async fn remove_session(&self, session_id: &str) -> bool {
        if session_id.is_empty() {
            return false;
        }

        let mut sessions = self.sessions.write().await;
        sessions.remove(session_id).is_some()
    }

    /// Clear all session affinity records
    pub async fn clear_all(&self) {
        let mut sessions = self.sessions.write().await;
        sessions.clear();
    }

    /// Get the number of active sessions
    pub async fn session_count(&self) -> usize {
        let sessions = self.sessions.read().await;
        sessions.len()
    }

    /// Cleanup expired sessions
    fn cleanup_expired(&self, sessions: &mut HashMap<String, SessionAffinity>) {
        let now = Utc::now();
        let ttl = chrono::Duration::seconds(self.session_ttl_seconds);

        // Collect expired sessions
        let expired: Vec<String> = sessions
            .iter()
            .filter(|(_, affinity)| now.signed_duration_since(affinity.last_access) > ttl)
            .map(|(id, _)| id.clone())
            .collect();

        // Remove expired sessions
        for id in expired {
            sessions.remove(&id);
        }

        // If still over limit, remove oldest sessions
        if sessions.len() > self.max_sessions {
            let mut entries: Vec<(String, DateTime<Utc>)> = sessions
                .iter()
                .map(|(id, affinity)| (id.clone(), affinity.last_access))
                .collect();

            entries.sort_by(|a, b| a.1.cmp(&b.1));

            let remove_count = sessions.len().saturating_sub(self.max_sessions);
            for (id, _) in entries.into_iter().take(remove_count) {
                sessions.remove(&id);
            }
        }
    }

    /// Force cleanup of expired sessions
    pub async fn cleanup(&self) {
        let mut sessions = self.sessions.write().await;
        self.cleanup_expired(&mut sessions);
    }

    /// Get all active session IDs
    pub async fn list_sessions(&self) -> Vec<String> {
        let sessions = self.sessions.read().await;
        sessions.keys().cloned().collect()
    }

    /// Get statistics about sessions
    pub async fn get_stats(&self) -> SessionStats {
        let sessions = self.sessions.read().await;

        let total_requests: u64 = sessions.values().map(|s| s.request_count).sum();
        let avg_requests = if sessions.is_empty() {
            0.0
        } else {
            total_requests as f64 / sessions.len() as f64
        };

        let providers_count: std::collections::HashMap<String, usize> =
            sessions
                .values()
                .fold(std::collections::HashMap::new(), |mut acc, affinity| {
                    *acc.entry(affinity.preferred_provider.clone()).or_insert(0) += 1;
                    acc
                });

        SessionStats {
            total_sessions: sessions.len(),
            total_requests,
            avg_requests_per_session: avg_requests,
            unique_providers: providers_count.len(),
            providers_distribution: providers_count,
        }
    }
}

/// Session statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStats {
    /// Total number of active sessions
    pub total_sessions: usize,
    /// Total requests across all sessions
    pub total_requests: u64,
    /// Average requests per session
    pub avg_requests_per_session: f64,
    /// Number of unique providers
    pub unique_providers: usize,
    /// Distribution of sessions per provider
    pub providers_distribution: HashMap<String, usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_set_and_get_provider() {
        let manager = SessionAffinityManager::new();

        // Set provider for a session
        manager
            .set_provider("session-1".to_string(), "provider-a".to_string())
            .await
            .unwrap();

        // Get preferred provider
        let provider = manager.get_preferred_provider("session-1").await;
        assert_eq!(provider, Some("provider-a".to_string()));
    }

    #[tokio::test]
    async fn test_new_session_any_provider() {
        let manager = SessionAffinityManager::new();

        // New session should have no affinity
        let provider = manager.get_preferred_provider("new-session").await;
        assert!(provider.is_none());
    }

    #[tokio::test]
    async fn test_existing_session_prefers_same_provider() {
        let manager = SessionAffinityManager::new();

        // Set initial provider
        manager
            .set_provider("session-1".to_string(), "provider-a".to_string())
            .await
            .unwrap();

        // Update with same provider
        manager
            .set_provider("session-1".to_string(), "provider-a".to_string())
            .await
            .unwrap();

        let affinity = manager.get_affinity("session-1").await;
        assert!(affinity.is_some());
        assert_eq!(affinity.unwrap().preferred_provider, "provider-a");
    }

    #[tokio::test]
    async fn test_update_provider() {
        let manager = SessionAffinityManager::new();

        // Set initial provider
        manager
            .set_provider("session-1".to_string(), "provider-a".to_string())
            .await
            .unwrap();

        // Update to different provider
        manager
            .set_provider("session-1".to_string(), "provider-b".to_string())
            .await
            .unwrap();

        let provider = manager.get_preferred_provider("session-1").await;
        assert_eq!(provider, Some("provider-b".to_string()));
    }

    #[tokio::test]
    async fn test_multi_turn_maintains_affinity() {
        let manager = SessionAffinityManager::new();

        let session_id = "multi-turn-session";
        let provider = "provider-x";

        // Simulate multiple turns
        for i in 0..5 {
            manager
                .set_provider(session_id.to_string(), provider.to_string())
                .await
                .unwrap();

            let affinity = manager.get_affinity(session_id).await;
            assert!(affinity.is_some());
            assert_eq!(affinity.unwrap().request_count, (i + 1) as u64);
        }

        // Final check
        let final_affinity = manager.get_affinity(session_id).await;
        assert_eq!(final_affinity.unwrap().request_count, 5);
    }

    #[tokio::test]
    async fn test_remove_session() {
        let manager = SessionAffinityManager::new();

        // Set provider
        manager
            .set_provider("session-1".to_string(), "provider-a".to_string())
            .await
            .unwrap();

        // Remove session
        assert!(manager.remove_session("session-1").await);

        // Session should be gone
        assert!(!manager.has_affinity("session-1").await);
        assert!(manager.get_preferred_provider("session-1").await.is_none());
    }

    #[tokio::test]
    async fn test_clear_all_sessions() {
        let manager = SessionAffinityManager::new();

        // Add multiple sessions
        for i in 0..5 {
            manager
                .set_provider(format!("session-{}", i), "provider-a".to_string())
                .await
                .unwrap();
        }

        assert_eq!(manager.session_count().await, 5);

        // Clear all
        manager.clear_all().await;

        assert_eq!(manager.session_count().await, 0);
    }

    #[tokio::test]
    async fn test_empty_session_id() {
        let manager = SessionAffinityManager::new();

        // Empty session ID should return None
        let provider = manager.get_preferred_provider("").await;
        assert!(provider.is_none());

        // Empty session ID should not have affinity
        assert!(!manager.has_affinity("").await);

        // Empty session ID cannot be set
        let result = manager
            .set_provider("".to_string(), "provider-a".to_string())
            .await;
        assert!(result.is_err());

        // Empty session ID cannot be removed
        assert!(!manager.remove_session("").await);
    }

    #[tokio::test]
    async fn test_empty_provider() {
        let manager = SessionAffinityManager::new();

        // Empty provider should error
        let result = manager
            .set_provider("session-1".to_string(), "".to_string())
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_list_sessions() {
        let manager = SessionAffinityManager::new();

        // Add sessions
        manager
            .set_provider("session-1".to_string(), "provider-a".to_string())
            .await
            .unwrap();
        manager
            .set_provider("session-2".to_string(), "provider-b".to_string())
            .await
            .unwrap();

        let sessions = manager.list_sessions().await;
        assert_eq!(sessions.len(), 2);
        assert!(sessions.contains(&"session-1".to_string()));
        assert!(sessions.contains(&"session-2".to_string()));
    }

    #[tokio::test]
    async fn test_session_stats() {
        let manager = SessionAffinityManager::new();

        // Add sessions with multiple providers
        for i in 0..3 {
            manager
                .set_provider(format!("session-{}", i), "provider-a".to_string())
                .await
                .unwrap();
        }
        for i in 3..5 {
            manager
                .set_provider(format!("session-{}", i), "provider-b".to_string())
                .await
                .unwrap();
        }

        let stats = manager.get_stats().await;
        assert_eq!(stats.total_sessions, 5);
        assert_eq!(stats.unique_providers, 2);
        assert_eq!(stats.total_requests, 5);
        assert!((stats.avg_requests_per_session - 1.0).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_cleanup_expired_sessions() {
        let manager = SessionAffinityManager::with_limits(100, 1); // 1 second TTL

        // Add sessions
        manager
            .set_provider("session-1".to_string(), "provider-a".to_string())
            .await
            .unwrap();

        // Wait for TTL to expire
        tokio::time::sleep(tokio::time::Duration::from_millis(1100)).await;

        // Trigger cleanup
        manager.cleanup().await;

        // Session should be removed
        assert!(!manager.has_affinity("session-1").await);
    }

    #[tokio::test]
    async fn test_max_sessions_limit() {
        let manager = SessionAffinityManager::with_limits(3, 3600); // Max 3 sessions

        // Add more sessions than limit
        for i in 0..5 {
            manager
                .set_provider(format!("session-{}", i), "provider-a".to_string())
                .await
                .unwrap();

            // Small delay to ensure different timestamps
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }

        // Should not exceed max limit
        assert!(manager.session_count().await <= 3);
    }

    #[tokio::test]
    async fn test_clone_shared_state() {
        let manager1 = SessionAffinityManager::new();

        // Add session to original
        manager1
            .set_provider("session-1".to_string(), "provider-a".to_string())
            .await
            .unwrap();

        let manager2 = manager1.clone();

        // Clone should share state via Arc
        let provider = manager2.get_preferred_provider("session-1").await;
        assert_eq!(provider, Some("provider-a".to_string()));
    }

    #[tokio::test]
    async fn test_request_count_increment() {
        let manager = SessionAffinityManager::new();

        // First request
        manager
            .set_provider("session-1".to_string(), "provider-a".to_string())
            .await
            .unwrap();

        let affinity = manager.get_affinity("session-1").await;
        assert_eq!(affinity.unwrap().request_count, 1);

        // Second request
        manager
            .set_provider("session-1".to_string(), "provider-a".to_string())
            .await
            .unwrap();

        let affinity = manager.get_affinity("session-1").await;
        assert_eq!(affinity.unwrap().request_count, 2);
    }

    #[tokio::test]
    async fn test_get_affinity_details() {
        let manager = SessionAffinityManager::new();

        manager
            .set_provider("session-1".to_string(), "provider-a".to_string())
            .await
            .unwrap();

        let affinity = manager.get_affinity("session-1").await;
        assert!(affinity.is_some());
        let details = affinity.unwrap();
        assert_eq!(details.session_id, "session-1");
        assert_eq!(details.preferred_provider, "provider-a");
        assert_eq!(details.request_count, 1);
        assert!(details.last_access <= Utc::now());
    }

    #[tokio::test]
    async fn test_providers_distribution() {
        let manager = SessionAffinityManager::new();

        // Add sessions to different providers
        manager
            .set_provider("session-1".to_string(), "provider-a".to_string())
            .await
            .unwrap();
        manager
            .set_provider("session-2".to_string(), "provider-a".to_string())
            .await
            .unwrap();
        manager
            .set_provider("session-3".to_string(), "provider-a".to_string())
            .await
            .unwrap();

        manager
            .set_provider("session-4".to_string(), "provider-b".to_string())
            .await
            .unwrap();
        manager
            .set_provider("session-5".to_string(), "provider-b".to_string())
            .await
            .unwrap();

        let stats = manager.get_stats().await;
        assert_eq!(stats.providers_distribution.get("provider-a"), Some(&3));
        assert_eq!(stats.providers_distribution.get("provider-b"), Some(&2));
    }
}
