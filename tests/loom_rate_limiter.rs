//! Loom-based systematic concurrency tests for RateLimiter.
//!
//! Tests ALL possible thread interleavings for the rate limiting logic.
//! Run with: cargo test --test loom_rate_limiter

use std::collections::HashMap;
use std::sync::Arc;

use loom::sync::Mutex;

/// Model of RateLimiter using loom's sync primitives.
struct LoomRateLimiter {
    buckets: Arc<Mutex<HashMap<String, (u64, u64)>>>,
    max_requests: u64,
}

impl LoomRateLimiter {
    fn new(max_requests: u64) -> Self {
        Self {
            buckets: Arc::new(Mutex::new(HashMap::new())),
            max_requests,
        }
    }

    fn check(&self, ip: &str) -> bool {
        let mut buckets = self.buckets.lock().unwrap();
        let (count, _window) = buckets.entry(ip.to_string()).or_insert((0, 0));

        if *count >= self.max_requests {
            false
        } else {
            *count = count.saturating_add(1);
            true
        }
    }
}

#[test]
fn rate_limiter_two_threads_no_exceed() {
    loom::model(|| {
        let limiter = Arc::new(LoomRateLimiter::new(2));
        let limiter1 = Arc::clone(&limiter);
        let limiter2 = Arc::clone(&limiter);

        let t1 = loom::thread::spawn(move || limiter1.check("192.168.1.1"));
        let t2 = loom::thread::spawn(move || limiter2.check("192.168.1.1"));

        let r1 = t1.join().unwrap();
        let r2 = t2.join().unwrap();

        assert!(r1);
        assert!(r2);
    });
}

#[test]
fn rate_limiter_three_threads_one_rejected() {
    loom::model(|| {
        let limiter = Arc::new(LoomRateLimiter::new(2));
        let limiter1 = Arc::clone(&limiter);
        let limiter2 = Arc::clone(&limiter);
        let limiter3 = Arc::clone(&limiter);

        let t1 = loom::thread::spawn(move || limiter1.check("10.0.0.1"));
        let t2 = loom::thread::spawn(move || limiter2.check("10.0.0.1"));
        let t3 = loom::thread::spawn(move || limiter3.check("10.0.0.1"));

        let r1 = t1.join().unwrap();
        let r2 = t2.join().unwrap();
        let r3 = t3.join().unwrap();

        let successes = [r1, r2, r3].iter().filter(|&&b| b).count();
        assert_eq!(successes, 2, "Exactly 2 of 3 requests should succeed");
    });
}

#[test]
fn rate_limiter_different_ips_independent() {
    loom::model(|| {
        let limiter = Arc::new(LoomRateLimiter::new(1));
        let limiter1 = Arc::clone(&limiter);
        let limiter2 = Arc::clone(&limiter);

        let t1 = loom::thread::spawn(move || limiter1.check("1.1.1.1"));
        let t2 = loom::thread::spawn(move || limiter2.check("2.2.2.2"));

        let r1 = t1.join().unwrap();
        let r2 = t2.join().unwrap();

        assert!(r1, "Different IPs should not interfere");
        assert!(r2, "Different IPs should not interfere");
    });
}

#[test]
fn rate_limiter_concurrent_prune_and_check() {
    loom::model(|| {
        let limiter = Arc::new(LoomRateLimiter::new(5));
        let limiter1 = Arc::clone(&limiter);
        let limiter2 = Arc::clone(&limiter);

        let t1 = loom::thread::spawn(move || {
            limiter1.check("prune-test");
        });

        let t2 = loom::thread::spawn(move || {
            let mut buckets = limiter2.buckets.lock().unwrap();
            buckets.retain(|_, _| true);
            drop(buckets);
        });

        let _ = t1.join().unwrap();
        let _ = t2.join().unwrap();
    });
}
