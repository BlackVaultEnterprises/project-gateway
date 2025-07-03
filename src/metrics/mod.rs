use metrics::{counter, histogram, Counter, Histogram};
use once_cell::sync::Lazy;

pub struct MirrorMetrics {
    pub requests_total: Counter,
    pub failures_total: Counter,
    pub latency_seconds: Histogram,
}

pub static MIRROR_METRICS: Lazy<MirrorMetrics> = Lazy::new(|| MirrorMetrics {
    requests_total: counter!("gateway_mirror_requests_total"),
    failures_total: counter!("gateway_mirror_failures_total"),
    latency_seconds: histogram!("gateway_mirror_latency_seconds"),
});

pub async fn metrics_handler() -> String {
    let encoder = prometheus::TextEncoder::new();
    let metric_families = prometheus::gather();
    
    encoder
        .encode_to_string(&metric_families)
        .unwrap_or_else(|_| "Error encoding metrics".to_string())
}

