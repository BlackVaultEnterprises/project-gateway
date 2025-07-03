use metrics::{counter, histogram, Counter, Histogram};
use once_cell::sync::Lazy;

pub struct MirrorMetrics {
    pub requests_total: Counter,
    pub failures_total: Counter,
    pub latency_seconds: Histogram,
}

pub struct GatewayMetrics {
    pub requests_total: Counter,
    pub errors_5xx_total: Counter,
    pub latency_seconds: Histogram,
    pub rust_requests_total: Counter,
    pub legacy_requests_total: Counter,
}

pub static MIRROR_METRICS: Lazy<MirrorMetrics> = Lazy::new(|| MirrorMetrics {
    requests_total: counter!("gateway_mirror_requests_total"),
    failures_total: counter!("gateway_mirror_failures_total"),
    latency_seconds: histogram!("gateway_mirror_latency_seconds"),
});

pub static GATEWAY_METRICS: Lazy<GatewayMetrics> = Lazy::new(|| GatewayMetrics {
    requests_total: counter!("gateway_requests_total"),
    errors_5xx_total: counter!("gateway_5xx_total"),
    latency_seconds: histogram!("gateway_latency_seconds"),
    rust_requests_total: counter!("gateway_rust_requests_total"),
    legacy_requests_total: counter!("gateway_legacy_requests_total"),
});

pub fn record_gateway_request(gateway_type: &str, status_code: u16, latency_seconds: f64) {
    // Record total requests
    GATEWAY_METRICS.requests_total.increment(1);
    
    // Record latency
    GATEWAY_METRICS.latency_seconds.record(latency_seconds);
    
    // Record 5xx errors
    if status_code >= 500 {
        GATEWAY_METRICS.errors_5xx_total.increment(1);
    }
    
    // Record by gateway type
    match gateway_type {
        "rust" => GATEWAY_METRICS.rust_requests_total.increment(1),
        "legacy" => GATEWAY_METRICS.legacy_requests_total.increment(1),
        _ => {}
    }
}

pub async fn metrics_handler() -> String {
    let encoder = prometheus::TextEncoder::new();
    let metric_families = prometheus::gather();
    
    encoder
        .encode_to_string(&metric_families)
        .unwrap_or_else(|_| "Error encoding metrics".to_string())
}

