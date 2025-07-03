use criterion::{black_box, criterion_group, criterion_main, Criterion};
use project_gateway::config::AppConfig;
use std::sync::Arc;
use tokio::runtime::Runtime;

fn config_loading_benchmark(c: &mut Criterion) {
    c.bench_function("config_loading", |b| {
        b.iter(|| black_box(AppConfig::load().unwrap()))
    });
}

fn json_serialization_benchmark(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("json_serialization", |b| {
        b.to_async(&rt).iter(|| async {
            let data = serde_json::json!({
                "status": "healthy",
                "service": "project-gateway",
                "version": "0.1.0",
                "timestamp": chrono::Utc::now().to_rfc3339()
            });
            black_box(serde_json::to_string(&data).unwrap())
        })
    });
}

fn uuid_generation_benchmark(c: &mut Criterion) {
    c.bench_function("uuid_generation", |b| {
        b.iter(|| black_box(uuid::Uuid::new_v4().to_string()))
    });
}

criterion_group!(
    benches,
    config_loading_benchmark,
    json_serialization_benchmark,
    uuid_generation_benchmark
);
criterion_main!(benches);
