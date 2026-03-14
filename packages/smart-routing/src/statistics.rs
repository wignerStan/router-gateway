use crate::outcome::{ErrorClass, ExecutionOutcome};
use chrono::{DateTime, Datelike, Duration, Timelike, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Smoothing factor for Exponentially Weighted Moving Average (EWMA) latency.
const EWMA_ALPHA: f64 = 0.2;

/// Start of peak hours (inclusive, 9 AM local time).
const PEAK_HOUR_START: u32 = 9;

/// End of peak hours (exclusive, 9 PM local time).
const PEAK_HOUR_END: u32 = 21;

/// Time bucket for statistics aggregation
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum TimeBucket {
    /// Peak hours (9 AM - 9 PM local time)
    Peak,
    /// Off-peak hours (9 PM - 9 AM local time)
    OffPeak,
    /// Weekday
    Weekday,
    /// Weekend
    Weekend,
    /// Compound: weekday during peak hours
    WeekdayPeak,
    /// Compound: weekday during off-peak hours
    WeekdayOffPeak,
    /// Compound: weekend during peak hours
    WeekendPeak,
    /// Compound: weekend during off-peak hours
    WeekendOffPeak,
    /// Hour of day (0-23)
    Hour(u8),
    /// Day of week (0=Sunday, 6=Saturday)
    DayOfWeek(u8),
}

impl TimeBucket {
    /// Get time bucket from timestamp
    pub fn from_timestamp(timestamp: DateTime<Utc>) -> Vec<Self> {
        let hour = timestamp.hour();
        let weekday = timestamp.weekday().num_days_from_sunday() as u8;
        let is_peak = (PEAK_HOUR_START..PEAK_HOUR_END).contains(&hour);
        let is_weekend = weekday == 0 || weekday == 6;

        let mut buckets = Vec::new();

        // Peak/off-peak
        if is_peak {
            buckets.push(Self::Peak);
        } else {
            buckets.push(Self::OffPeak);
        }

        // Weekday/weekend
        if is_weekend {
            buckets.push(Self::Weekend);
        } else {
            buckets.push(Self::Weekday);
        }

        // Compound buckets: weekday/weekend + peak/off-peak
        match (is_weekend, is_peak) {
            (false, true) => buckets.push(Self::WeekdayPeak),
            (false, false) => buckets.push(Self::WeekdayOffPeak),
            (true, true) => buckets.push(Self::WeekendPeak),
            (true, false) => buckets.push(Self::WeekendOffPeak),
        }

        // Specific hour
        buckets.push(Self::Hour(hour as u8));

        // Specific day
        buckets.push(Self::DayOfWeek(weekday));

        buckets
    }

    /// Get peak/off-peak bucket
    pub fn peak_off_peak(timestamp: DateTime<Utc>) -> Self {
        let hour = timestamp.hour();
        if (PEAK_HOUR_START..PEAK_HOUR_END).contains(&hour) {
            Self::Peak
        } else {
            Self::OffPeak
        }
    }

    /// Get weekday/weekend bucket
    pub fn weekday_weekend(timestamp: DateTime<Utc>) -> Self {
        let weekday = timestamp.weekday().num_days_from_sunday();
        if weekday == 0 || weekday == 6 {
            Self::Weekend
        } else {
            Self::Weekday
        }
    }
}

/// Statistics for a specific time bucket
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BucketStatistics {
    /// Total requests
    pub total_requests: u64,
    /// Successful requests
    pub success_count: u64,
    /// Failed requests
    pub failure_count: u64,
    /// Success rate
    pub success_rate: f64,
    /// Average latency in milliseconds
    pub avg_latency_ms: f64,
    /// P50 latency
    pub p50_latency_ms: f64,
    /// P95 latency
    pub p95_latency_ms: f64,
    /// P99 latency
    pub p99_latency_ms: f64,
    /// Minimum latency
    pub min_latency_ms: f64,
    /// Maximum latency
    pub max_latency_ms: f64,
    /// Total tokens
    pub total_tokens: u64,
    /// Average tokens per request
    pub avg_tokens: f64,
    /// Error counts by class
    pub error_counts: HashMap<ErrorClass, u64>,
    /// Fallback usage count
    pub fallback_count: u64,
    /// Last update timestamp
    pub last_updated: DateTime<Utc>,
}

impl Default for BucketStatistics {
    fn default() -> Self {
        Self {
            total_requests: 0,
            success_count: 0,
            failure_count: 0,
            success_rate: 1.0,
            avg_latency_ms: 0.0,
            p50_latency_ms: 0.0,
            p95_latency_ms: 0.0,
            p99_latency_ms: 0.0,
            min_latency_ms: f64::MAX,
            max_latency_ms: 0.0,
            total_tokens: 0,
            avg_tokens: 0.0,
            error_counts: HashMap::new(),
            fallback_count: 0,
            last_updated: Utc::now(),
        }
    }
}

/// Aggregated statistics for a route
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteStatistics {
    /// Route ID
    pub route_id: String,
    /// Overall statistics
    pub overall: BucketStatistics,
    /// Time-bucketed statistics
    pub time_buckets: HashMap<TimeBucket, BucketStatistics>,
    /// Created timestamp
    pub created_at: DateTime<Utc>,
    /// Last updated timestamp
    pub updated_at: DateTime<Utc>,
}

fn update_bucket_stats(stats: &mut BucketStatistics, outcome: &ExecutionOutcome) {
    stats.total_requests += 1;
    stats.last_updated = Utc::now();

    if outcome.success {
        stats.success_count += 1;
    } else {
        stats.failure_count += 1;
        if let Some(error_class) = outcome.error_class {
            *stats.error_counts.entry(error_class).or_insert(0) += 1;
        }
    }

    if outcome.used_fallback {
        stats.fallback_count += 1;
    }

    if stats.total_requests > 0 {
        stats.success_rate = stats.success_count as f64 / stats.total_requests as f64;
    }

    let latency = outcome.latency_ms;
    if latency > 0.0 {
        if stats.avg_latency_ms == 0.0 {
            stats.avg_latency_ms = latency;
        } else {
            // EWMA smoothing
            stats.avg_latency_ms =
                EWMA_ALPHA.mul_add(latency, (1.0 - EWMA_ALPHA) * stats.avg_latency_ms);
        }

        if latency < stats.min_latency_ms {
            stats.min_latency_ms = latency;
        }
        if latency > stats.max_latency_ms {
            stats.max_latency_ms = latency;
        }
    }

    stats.total_tokens += u64::from(outcome.total_tokens);
    if stats.total_requests > 0 {
        stats.avg_tokens = stats.total_tokens as f64 / stats.total_requests as f64;
    }
}

impl RouteStatistics {
    /// Create new route statistics
    pub fn new(route_id: String) -> Self {
        let now = Utc::now();
        Self {
            route_id,
            overall: BucketStatistics::default(),
            time_buckets: HashMap::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Update statistics with an outcome
    pub fn update(&mut self, outcome: &ExecutionOutcome) {
        update_bucket_stats(&mut self.overall, outcome);
        let buckets = TimeBucket::from_timestamp(outcome.timestamp);
        for bucket in buckets {
            let stats = self.time_buckets.entry(bucket).or_default();
            update_bucket_stats(stats, outcome);
        }
        self.updated_at = Utc::now();
    }

    /// Get statistics for a specific time bucket
    pub fn get_bucket_stats(&self, bucket: &TimeBucket) -> Option<&BucketStatistics> {
        self.time_buckets.get(bucket)
    }

    /// Get all time bucket statistics
    pub const fn get_all_buckets(&self) -> &HashMap<TimeBucket, BucketStatistics> {
        &self.time_buckets
    }
}

/// Cold start priors for new routes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColdStartPriors {
    /// Provider-level priors
    pub provider_priors: HashMap<String, BucketStatistics>,
    /// Tier-level priors
    pub tier_priors: HashMap<String, BucketStatistics>,
    /// Default neutral prior
    pub neutral_prior: BucketStatistics,
}

impl Default for ColdStartPriors {
    fn default() -> Self {
        let neutral_prior = BucketStatistics {
            total_requests: 10,
            success_count: 9,
            failure_count: 1,
            success_rate: 0.9,
            avg_latency_ms: 500.0,
            p50_latency_ms: 400.0,
            p95_latency_ms: 800.0,
            p99_latency_ms: 1200.0,
            min_latency_ms: 100.0,
            max_latency_ms: 2000.0,
            total_tokens: 5000,
            avg_tokens: 500.0,
            error_counts: HashMap::new(),
            fallback_count: 0,
            last_updated: Utc::now(),
        };

        Self {
            provider_priors: HashMap::new(),
            tier_priors: HashMap::new(),
            neutral_prior,
        }
    }
}

impl ColdStartPriors {
    /// Create new cold start priors
    pub fn new() -> Self {
        Self::default()
    }

    /// Set provider prior
    pub fn set_provider_prior(&mut self, provider: String, prior: BucketStatistics) {
        self.provider_priors.insert(provider, prior);
    }

    /// Set tier prior
    pub fn set_tier_prior(&mut self, tier: String, prior: BucketStatistics) {
        self.tier_priors.insert(tier, prior);
    }

    /// Get prior for a route
    pub fn get_prior(&self, provider: Option<&str>, tier: Option<&str>) -> BucketStatistics {
        // Try provider prior first
        if let Some(p) = provider {
            if let Some(prior) = self.provider_priors.get(p) {
                return prior.clone();
            }
        }

        // Try tier prior
        if let Some(t) = tier {
            if let Some(prior) = self.tier_priors.get(t) {
                return prior.clone();
            }
        }

        // Fall back to neutral prior
        self.neutral_prior.clone()
    }

    /// Initialize route statistics with priors
    pub fn initialize_route(
        &self,
        route_id: String,
        provider: Option<&str>,
        tier: Option<&str>,
    ) -> RouteStatistics {
        let prior = self.get_prior(provider, tier);
        let mut stats = RouteStatistics::new(route_id);
        stats.overall = prior;
        stats
    }
}

/// Statistics aggregator
#[derive(Debug, Clone)]
pub struct StatisticsAggregator {
    /// Route statistics
    pub route_stats: HashMap<String, RouteStatistics>,
    /// Cold start priors
    pub priors: ColdStartPriors,
    /// Maximum age for statistics before cleanup
    pub max_age_days: i64,
}

impl StatisticsAggregator {
    /// Create a new statistics aggregator
    pub fn new() -> Self {
        Self {
            route_stats: HashMap::new(),
            priors: ColdStartPriors::new(),
            max_age_days: 30,
        }
    }

    /// Create with custom priors
    pub fn with_priors(priors: ColdStartPriors) -> Self {
        Self {
            route_stats: HashMap::new(),
            priors,
            max_age_days: 30,
        }
    }

    /// Record an execution outcome
    pub fn record(&mut self, outcome: &ExecutionOutcome) {
        let route_id = outcome.effective_route().to_string();

        use std::collections::hash_map::Entry;
        let stats = match self.route_stats.entry(route_id) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                let route_id = entry.key().clone();
                entry.insert(RouteStatistics::new(route_id))
            },
        };

        stats.update(outcome);
    }

    /// Get statistics for a route
    pub fn get_stats(&self, route_id: &str) -> Option<&RouteStatistics> {
        self.route_stats.get(route_id)
    }

    /// Get all route statistics
    pub const fn get_all_stats(&self) -> &HashMap<String, RouteStatistics> {
        &self.route_stats
    }

    /// Initialize route with cold start priors
    pub fn initialize_route(
        &mut self,
        route_id: String,
        provider: Option<String>,
        tier: Option<String>,
    ) {
        use std::collections::hash_map::Entry;
        if let Entry::Vacant(entry) = self.route_stats.entry(route_id) {
            let route_id = entry.key().clone();
            let stats =
                self.priors
                    .initialize_route(route_id, provider.as_deref(), tier.as_deref());
            entry.insert(stats);
        }
    }

    /// Clean up old statistics
    pub fn cleanup_old(&mut self) {
        let cutoff = Utc::now() - Duration::days(self.max_age_days);
        self.route_stats
            .retain(|_, stats| stats.updated_at > cutoff);
    }

    /// Reset statistics for a route
    pub fn reset_route(&mut self, route_id: &str) {
        self.route_stats.remove(route_id);
    }

    /// Reset all statistics
    pub fn reset_all(&mut self) {
        self.route_stats.clear();
    }
}

impl Default for StatisticsAggregator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_bucket_from_timestamp() {
        // Monday at 10 AM (peak, weekday)
        let timestamp = DateTime::parse_from_rfc3339("2024-01-08T10:00:00Z")
            .expect("Operation should succeed during test")
            .with_timezone(&Utc);

        let buckets = TimeBucket::from_timestamp(timestamp);
        assert!(buckets.contains(&TimeBucket::Peak));
        assert!(buckets.contains(&TimeBucket::Weekday));
        assert!(buckets.contains(&TimeBucket::Hour(10)));
        assert!(buckets.contains(&TimeBucket::DayOfWeek(1)));
    }

    #[test]
    fn test_time_bucket_weekend() {
        // Saturday at 10 AM (peak, weekend)
        let timestamp = DateTime::parse_from_rfc3339("2024-01-06T10:00:00Z")
            .expect("Operation should succeed during test")
            .with_timezone(&Utc);

        let buckets = TimeBucket::from_timestamp(timestamp);
        assert!(buckets.contains(&TimeBucket::Peak));
        assert!(buckets.contains(&TimeBucket::Weekend));
        assert!(buckets.contains(&TimeBucket::DayOfWeek(6)));
    }

    #[test]
    fn test_time_bucket_off_peak() {
        // Monday at 2 AM (off-peak, weekday)
        let timestamp = DateTime::parse_from_rfc3339("2024-01-08T02:00:00Z")
            .expect("Operation should succeed during test")
            .with_timezone(&Utc);

        let buckets = TimeBucket::from_timestamp(timestamp);
        assert!(buckets.contains(&TimeBucket::OffPeak));
        assert!(buckets.contains(&TimeBucket::Weekday));
        assert!(buckets.contains(&TimeBucket::Hour(2)));
    }

    #[test]
    fn test_route_statistics_update() {
        let mut stats = RouteStatistics::new("route-1".to_string());

        let outcome1 = ExecutionOutcome::success("route-1".to_string(), 100.0, 10, 5, 200);
        let outcome2 = ExecutionOutcome::failure("route-1".to_string(), 200.0, 500, false, None);

        stats.update(&outcome1);
        stats.update(&outcome2);

        assert_eq!(stats.overall.total_requests, 2);
        assert_eq!(stats.overall.success_count, 1);
        assert_eq!(stats.overall.failure_count, 1);
        assert_eq!(stats.overall.success_rate, 0.5);
        assert!(stats.overall.avg_latency_ms > 0.0);
    }

    #[test]
    fn test_route_statistics_time_buckets() {
        let mut stats = RouteStatistics::new("route-1".to_string());

        let timestamp = DateTime::parse_from_rfc3339("2024-01-08T10:00:00Z")
            .expect("Operation should succeed during test")
            .with_timezone(&Utc);

        let mut outcome = ExecutionOutcome::success("route-1".to_string(), 100.0, 10, 5, 200);
        outcome.timestamp = timestamp;

        stats.update(&outcome);

        // Should have created time buckets
        assert!(stats.get_bucket_stats(&TimeBucket::Peak).is_some());
        assert!(stats.get_bucket_stats(&TimeBucket::Weekday).is_some());
        assert!(stats.get_bucket_stats(&TimeBucket::Hour(10)).is_some());
        assert!(stats.get_bucket_stats(&TimeBucket::DayOfWeek(1)).is_some());
    }

    #[test]
    fn test_cold_start_priors() {
        let mut priors = ColdStartPriors::new();

        let provider_prior = BucketStatistics {
            total_requests: 100,
            success_count: 95,
            failure_count: 5,
            success_rate: 0.95,
            avg_latency_ms: 300.0,
            p50_latency_ms: 250.0,
            p95_latency_ms: 500.0,
            p99_latency_ms: 700.0,
            min_latency_ms: 100.0,
            max_latency_ms: 1000.0,
            total_tokens: 50000,
            avg_tokens: 500.0,
            error_counts: HashMap::new(),
            fallback_count: 0,
            last_updated: Utc::now(),
        };

        priors.set_provider_prior("anthropic".to_string(), provider_prior);

        let stats = priors.initialize_route("route-1".to_string(), Some("anthropic"), None);
        assert_eq!(stats.overall.total_requests, 100);
        assert_eq!(stats.overall.success_rate, 0.95);
    }

    #[test]
    fn test_cold_start_priors_fallback() {
        let priors = ColdStartPriors::new();

        // No provider or tier - should use neutral prior
        let stats = priors.initialize_route("route-1".to_string(), None, None);
        assert_eq!(stats.overall.total_requests, 10);
        assert_eq!(stats.overall.success_rate, 0.9);
    }

    #[test]
    fn test_statistics_aggregator() {
        let mut aggregator = StatisticsAggregator::new();

        let outcome = ExecutionOutcome::success("route-1".to_string(), 100.0, 10, 5, 200);
        aggregator.record(&outcome);

        assert!(aggregator.get_stats("route-1").is_some());
        assert_eq!(
            aggregator
                .get_stats("route-1")
                .expect("Operation should succeed during test")
                .overall
                .total_requests,
            1
        );
    }

    #[test]
    fn test_statistics_aggregator_initialize_route() {
        let mut aggregator = StatisticsAggregator::new();

        aggregator.initialize_route("route-1".to_string(), Some("anthropic".to_string()), None);

        assert!(aggregator.get_stats("route-1").is_some());
    }

    #[test]
    fn test_statistics_aggregator_cleanup() {
        let mut aggregator = StatisticsAggregator::new();
        aggregator.max_age_days = 1;

        let mut old_stats = RouteStatistics::new("old-route".to_string());
        old_stats.updated_at = Utc::now() - Duration::days(2);
        aggregator
            .route_stats
            .insert("old-route".to_string(), old_stats);

        let mut new_stats = RouteStatistics::new("new-route".to_string());
        new_stats.updated_at = Utc::now();
        aggregator
            .route_stats
            .insert("new-route".to_string(), new_stats);

        aggregator.cleanup_old();

        assert!(aggregator.get_stats("old-route").is_none());
        assert!(aggregator.get_stats("new-route").is_some());
    }

    #[test]
    fn test_statistics_aggregator_reset() {
        let mut aggregator = StatisticsAggregator::new();

        let outcome = ExecutionOutcome::success("route-1".to_string(), 100.0, 10, 5, 200);
        aggregator.record(&outcome);

        aggregator.reset_route("route-1");
        assert!(aggregator.get_stats("route-1").is_none());

        aggregator.record(&outcome);
        aggregator.reset_all();
        assert!(aggregator.get_stats("route-1").is_none());
    }

    #[test]
    fn test_bucket_statistics_error_counts() {
        let mut stats = RouteStatistics::new("route-1".to_string());

        let outcome1 = ExecutionOutcome::failure("route-1".to_string(), 200.0, 429, false, None);
        let outcome2 = ExecutionOutcome::failure("route-1".to_string(), 200.0, 500, false, None);
        let outcome3 = ExecutionOutcome::failure("route-1".to_string(), 200.0, 429, false, None);

        stats.update(&outcome1);
        stats.update(&outcome2);
        stats.update(&outcome3);

        assert_eq!(
            stats
                .overall
                .error_counts
                .get(&ErrorClass::RateLimit)
                .copied()
                .unwrap_or(0),
            2
        );
        assert_eq!(
            stats
                .overall
                .error_counts
                .get(&ErrorClass::ServerError)
                .copied()
                .unwrap_or(0),
            1
        );
    }

    #[test]
    fn test_bucket_statistics_fallback_tracking() {
        let mut stats = RouteStatistics::new("route-1".to_string());

        let outcome = ExecutionOutcome::failure(
            "route-fallback".to_string(),
            200.0,
            500,
            true,
            Some("route-original".to_string()),
        );

        stats.update(&outcome);
        assert_eq!(stats.overall.fallback_count, 1);
    }

    // ========================================
    // Compound Weekend/Weekday Bucket Tests
    // ========================================

    #[test]
    fn test_compound_buckets_isolate_weekday_peak() {
        // Monday 10:00 UTC -> weekday + peak
        let ts = DateTime::parse_from_rfc3339("2026-03-09T10:00:00Z")
            .expect("Operation should succeed during test")
            .with_timezone(&Utc);
        let buckets = TimeBucket::from_timestamp(ts);
        assert!(
            buckets.contains(&TimeBucket::WeekdayPeak),
            "Monday 10am should be WeekdayPeak"
        );
        assert!(!buckets.contains(&TimeBucket::WeekdayOffPeak));
        assert!(!buckets.contains(&TimeBucket::WeekendPeak));
        assert!(!buckets.contains(&TimeBucket::WeekendOffPeak));
    }

    #[test]
    fn test_compound_buckets_isolate_weekday_offpeak() {
        // Tuesday 03:00 UTC -> weekday + off-peak
        let ts = DateTime::parse_from_rfc3339("2026-03-10T03:00:00Z")
            .expect("Operation should succeed during test")
            .with_timezone(&Utc);
        let buckets = TimeBucket::from_timestamp(ts);
        assert!(
            buckets.contains(&TimeBucket::WeekdayOffPeak),
            "Tuesday 3am should be WeekdayOffPeak"
        );
        assert!(!buckets.contains(&TimeBucket::WeekdayPeak));
        assert!(!buckets.contains(&TimeBucket::WeekendPeak));
        assert!(!buckets.contains(&TimeBucket::WeekendOffPeak));
    }

    #[test]
    fn test_compound_buckets_isolate_weekend_peak() {
        // Saturday 14:00 UTC -> weekend + peak
        let ts = DateTime::parse_from_rfc3339("2026-03-14T14:00:00Z")
            .expect("Operation should succeed during test")
            .with_timezone(&Utc);
        let buckets = TimeBucket::from_timestamp(ts);
        assert!(
            buckets.contains(&TimeBucket::WeekendPeak),
            "Saturday 2pm should be WeekendPeak"
        );
        assert!(!buckets.contains(&TimeBucket::WeekendOffPeak));
        assert!(!buckets.contains(&TimeBucket::WeekdayPeak));
        assert!(!buckets.contains(&TimeBucket::WeekdayOffPeak));
    }

    #[test]
    fn test_compound_buckets_isolate_weekend_offpeak() {
        // Sunday 02:00 UTC -> weekend + off-peak
        let ts = DateTime::parse_from_rfc3339("2026-03-15T02:00:00Z")
            .expect("Operation should succeed during test")
            .with_timezone(&Utc);
        let buckets = TimeBucket::from_timestamp(ts);
        assert!(
            buckets.contains(&TimeBucket::WeekendOffPeak),
            "Sunday 2am should be WeekendOffPeak"
        );
        assert!(!buckets.contains(&TimeBucket::WeekendPeak));
        assert!(!buckets.contains(&TimeBucket::WeekdayPeak));
        assert!(!buckets.contains(&TimeBucket::WeekdayOffPeak));
    }

    #[test]
    fn test_weekday_peak_boundary_hours() {
        // Monday 08:59 -> weekday off-peak (just before peak)
        let ts = DateTime::parse_from_rfc3339("2026-03-09T08:59:00Z")
            .expect("Operation should succeed during test")
            .with_timezone(&Utc);
        let buckets = TimeBucket::from_timestamp(ts);
        assert!(buckets.contains(&TimeBucket::WeekdayOffPeak));
        assert!(buckets.contains(&TimeBucket::OffPeak));

        // Monday 09:00 -> weekday peak (peak starts)
        let ts = DateTime::parse_from_rfc3339("2026-03-09T09:00:00Z")
            .expect("Operation should succeed during test")
            .with_timezone(&Utc);
        let buckets = TimeBucket::from_timestamp(ts);
        assert!(buckets.contains(&TimeBucket::WeekdayPeak));
        assert!(buckets.contains(&TimeBucket::Peak));

        // Monday 20:59 -> weekday peak (just before off-peak)
        let ts = DateTime::parse_from_rfc3339("2026-03-09T20:59:00Z")
            .expect("Operation should succeed during test")
            .with_timezone(&Utc);
        let buckets = TimeBucket::from_timestamp(ts);
        assert!(buckets.contains(&TimeBucket::WeekdayPeak));

        // Monday 21:00 -> weekday off-peak (off-peak starts)
        let ts = DateTime::parse_from_rfc3339("2026-03-09T21:00:00Z")
            .expect("Operation should succeed during test")
            .with_timezone(&Utc);
        let buckets = TimeBucket::from_timestamp(ts);
        assert!(buckets.contains(&TimeBucket::WeekdayOffPeak));
    }

    #[test]
    fn test_stats_recorded_to_correct_compound_buckets() {
        let mut stats = RouteStatistics::new("route-1".to_string());

        // Record a weekday peak event (Monday 10am)
        let outcome = ExecutionOutcome::success("route-1".to_string(), 100.0, 200, 300, 200);
        let ts = DateTime::parse_from_rfc3339("2026-03-09T10:00:00Z")
            .expect("Operation should succeed during test")
            .with_timezone(&Utc);
        let mut outcome_wp = outcome.clone();
        outcome_wp.timestamp = ts;
        stats.update(&outcome_wp);

        // Record a weekend off-peak event (Sunday 2am)
        let mut outcome_wo = outcome;
        outcome_wo.timestamp = DateTime::parse_from_rfc3339("2026-03-15T02:00:00Z")
            .expect("Operation should succeed during test")
            .with_timezone(&Utc);
        stats.update(&outcome_wo);

        // Verify compound buckets are independently populated
        assert_eq!(
            stats
                .get_bucket_stats(&TimeBucket::WeekdayPeak)
                .expect("Operation should succeed during test")
                .total_requests,
            1,
            "WeekdayPeak should have exactly 1 request"
        );
        assert_eq!(
            stats
                .get_bucket_stats(&TimeBucket::WeekendOffPeak)
                .expect("Operation should succeed during test")
                .total_requests,
            1,
            "WeekendOffPeak should have exactly 1 request"
        );
        // Verify compound buckets that had no events return None or 0
        assert_eq!(
            stats
                .get_bucket_stats(&TimeBucket::WeekdayOffPeak)
                .map_or(0, |s| s.total_requests),
            0,
            "WeekdayOffPeak should have 0 requests"
        );
        assert_eq!(
            stats
                .get_bucket_stats(&TimeBucket::WeekendPeak)
                .map_or(0, |s| s.total_requests),
            0,
            "WeekendPeak should have 0 requests"
        );

        // Verify parent buckets aggregate correctly
        assert_eq!(
            stats
                .get_bucket_stats(&TimeBucket::Weekday)
                .expect("Operation should succeed during test")
                .total_requests,
            1,
            "Weekday should aggregate all weekday requests"
        );
        assert_eq!(
            stats
                .get_bucket_stats(&TimeBucket::Weekend)
                .expect("Operation should succeed during test")
                .total_requests,
            1,
            "Weekend should aggregate all weekend requests"
        );
        assert_eq!(
            stats
                .get_bucket_stats(&TimeBucket::Peak)
                .expect("Operation should succeed during test")
                .total_requests,
            1,
            "Peak should aggregate all peak requests"
        );
    }
}
