#![allow(clippy::semicolon_if_nothing_returned, missing_docs)]
use chrono::Utc;
use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use gateway::routing::config::WeightConfig;
use gateway::routing::health::HealthStatus;
use gateway::routing::metrics::AuthMetrics;
use gateway::routing::weight::{AuthInfo, DefaultWeightCalculator, ModelState, WeightCalculator};

fn healthy_auth() -> AuthInfo {
    AuthInfo {
        id: "bench-cred-1".to_string(),
        priority: Some(50),
        quota_exceeded: false,
        unavailable: false,
        model_states: vec![],
    }
}

fn healthy_metrics() -> AuthMetrics {
    AuthMetrics {
        total_requests: 10_000,
        success_count: 9_500,
        failure_count: 500,
        avg_latency_ms: 120.0,
        min_latency_ms: 50.0,
        max_latency_ms: 800.0,
        success_rate: 0.95,
        error_rate: 0.05,
        consecutive_successes: 100,
        consecutive_failures: 0,
        last_request_time: Utc::now(),
        last_success_time: Some(Utc::now()),
        last_failure_time: None,
    }
}

fn sparse_metrics() -> AuthMetrics {
    AuthMetrics {
        total_requests: 3,
        success_count: 2,
        failure_count: 1,
        avg_latency_ms: 200.0,
        min_latency_ms: 150.0,
        max_latency_ms: 300.0,
        success_rate: 0.66,
        error_rate: 0.34,
        consecutive_successes: 1,
        consecutive_failures: 0,
        last_request_time: Utc::now(),
        last_success_time: Some(Utc::now()),
        last_failure_time: None,
    }
}

fn quota_exceeded_auth() -> AuthInfo {
    AuthInfo {
        id: "bench-cred-quota".to_string(),
        priority: Some(10),
        quota_exceeded: true,
        unavailable: false,
        model_states: vec![
            ModelState {
                unavailable: true,
                quota_exceeded: false,
            },
            ModelState {
                unavailable: false,
                quota_exceeded: true,
            },
            ModelState {
                unavailable: true,
                quota_exceeded: true,
            },
        ],
    }
}

fn bench_weight_calculation(c: &mut Criterion) {
    let calc = DefaultWeightCalculator::new(WeightConfig::default());
    let auth = healthy_auth();
    let metrics = healthy_metrics();

    let mut group = c.benchmark_group("weight_calculation");

    group.bench_function("healthy_full_metrics", |b| {
        b.iter(|| {
            calc.calculate(
                black_box(&auth),
                black_box(Some(&metrics)),
                black_box(HealthStatus::Healthy),
            )
        })
    });

    group.bench_function("degraded_sparse_metrics", |b| {
        let sparse = sparse_metrics();
        b.iter(|| {
            calc.calculate(
                black_box(&auth),
                black_box(Some(&sparse)),
                black_box(HealthStatus::Degraded),
            )
        })
    });

    group.bench_function("unhealthy_no_metrics", |b| {
        b.iter(|| {
            calc.calculate(
                black_box(&auth),
                black_box(None),
                black_box(HealthStatus::Unhealthy),
            )
        })
    });

    group.bench_function("quota_exceeded_with_model_states", |b| {
        let quota_auth = quota_exceeded_auth();
        b.iter(|| {
            calc.calculate(
                black_box(&quota_auth),
                black_box(Some(&metrics)),
                black_box(HealthStatus::Healthy),
            )
        })
    });

    group.finish();
}

fn bench_weight_calculation_across_health(c: &mut Criterion) {
    let calc = DefaultWeightCalculator::new(WeightConfig::default());
    let auth = healthy_auth();
    let metrics = healthy_metrics();

    let mut group = c.benchmark_group("weight_by_health_status");

    for (label, status) in [
        ("healthy", HealthStatus::Healthy),
        ("degraded", HealthStatus::Degraded),
        ("unhealthy", HealthStatus::Unhealthy),
    ] {
        group.bench_with_input(
            BenchmarkId::new("calculate", label),
            &status,
            |b, &status| {
                b.iter(|| {
                    calc.calculate(
                        black_box(&auth),
                        black_box(Some(&metrics)),
                        black_box(status),
                    )
                })
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_weight_calculation,
    bench_weight_calculation_across_health
);
criterion_main!(benches);
