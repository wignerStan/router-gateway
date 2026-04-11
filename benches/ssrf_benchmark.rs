use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use gateway::utils::ssrf::validate_url_not_private;

fn bench_validate_url(c: &mut Criterion) {
    let mut group = c.benchmark_group("ssrf_validate_url");

    group.bench_function("public_domain", |b| {
        b.iter(|| validate_url_not_private(black_box("https://api.openai.com/v1/chat/completions")))
    });

    group.bench_function("public_ip_literal", |b| {
        b.iter(|| validate_url_not_private(black_box("https://1.1.1.1/api")))
    });

    group.bench_function("private_ip_rejected", |b| {
        b.iter(|| validate_url_not_private(black_box("http://192.168.1.1/api")))
    });

    group.bench_function("ipv4_mapped_ipv6", |b| {
        b.iter(|| validate_url_not_private(black_box("http://[::ffff:127.0.0.1]:8000/api")))
    });

    group.bench_function("invalid_url", |b| {
        b.iter(|| validate_url_not_private(black_box("not a url")))
    });

    group.bench_function("ipv6_loopback", |b| {
        b.iter(|| validate_url_not_private(black_box("http://[::1]:8000/api")))
    });

    group.bench_function("cloud_metadata", |b| {
        b.iter(|| validate_url_not_private(black_box("http://169.254.169.254/latest/meta-data/")))
    });

    group.finish();
}

fn bench_validate_url_by_type(c: &mut Criterion) {
    let urls = [
        ("domain", "https://api.anthropic.com/v1/messages"),
        ("ipv4_public", "https://8.8.8.8/v1/chat"),
        ("ipv4_private", "http://10.0.0.1/api"),
        ("ipv6_public", "https://[2606:4700:4700::1111]/api"),
        ("no_host", "file:///path/to/file"),
    ];

    let mut group = c.benchmark_group("ssrf_url_types");

    for (label, url) in &urls {
        group.bench_with_input(BenchmarkId::new("validate", label), url, |b, url| {
            b.iter(|| validate_url_not_private(black_box(url)))
        });
    }

    group.finish();
}

criterion_group!(benches, bench_validate_url, bench_validate_url_by_type);
criterion_main!(benches);
