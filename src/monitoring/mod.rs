use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use serde::{Deserialize, Serialize};
use tokio::time::interval;
use tracing::{info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub p99_latency_ms: f64,
    pub p95_latency_ms: f64,
    pub p50_latency_ms: f64,
    pub avg_latency_ms: f64,
    pub request_count: u64,
    pub error_rate: f64,
    pub cpu_usage_percent: f64,
    pub memory_usage_mb: f64,
    pub timestamp: u64,
}

#[derive(Debug, Clone)]
pub struct PerformanceBaseline {
    pub rust_metrics: PerformanceMetrics,
    pub legacy_metrics: PerformanceMetrics,
    pub improvement_factor: f64,
}

pub struct PerformanceMonitor {
    rust_latencies: Arc<Mutex<Vec<f64>>>,
    legacy_latencies: Arc<Mutex<Vec<f64>>>,
    rust_errors: Arc<Mutex<u64>>,
    legacy_errors: Arc<Mutex<u64>>,
    rust_requests: Arc<Mutex<u64>>,
    legacy_requests: Arc<Mutex<u64>>,
    baseline: Arc<Mutex<Option<PerformanceBaseline>>>,
}

impl PerformanceMonitor {
    pub fn new() -> Self {
        Self {
            rust_latencies: Arc::new(Mutex::new(Vec::new())),
            legacy_latencies: Arc::new(Mutex::new(Vec::new())),
            rust_errors: Arc::new(Mutex::new(0)),
            legacy_errors: Arc::new(Mutex::new(0)),
            rust_requests: Arc::new(Mutex::new(0)),
            legacy_requests: Arc::new(Mutex::new(0)),
            baseline: Arc::new(Mutex::new(None)),
        }
    }

    pub fn record_request(&self, gateway_type: &str, latency_ms: f64, is_error: bool) {
        match gateway_type {
            "rust" => {
                if let Ok(mut latencies) = self.rust_latencies.lock() {
                    latencies.push(latency_ms);
                    // Keep only last 1000 measurements for memory efficiency
                    if latencies.len() > 1000 {
                        let len = latencies.len();
                        latencies.drain(0..len - 1000);
                    }
                }
                if let Ok(mut requests) = self.rust_requests.lock() {
                    *requests += 1;
                }
                if is_error {
                    if let Ok(mut errors) = self.rust_errors.lock() {
                        *errors += 1;
                    }
                }
            }
            "legacy" => {
                if let Ok(mut latencies) = self.legacy_latencies.lock() {
                    latencies.push(latency_ms);
                    if latencies.len() > 1000 {
                        let len = latencies.len();
                        latencies.drain(0..len - 1000);
                    }
                }
                if let Ok(mut requests) = self.legacy_requests.lock() {
                    *requests += 1;
                }
                if is_error {
                    if let Ok(mut errors) = self.legacy_errors.lock() {
                        *errors += 1;
                    }
                }
            }
            _ => {}
        }
    }

    pub fn get_current_metrics(&self, gateway_type: &str) -> Option<PerformanceMetrics> {
        match gateway_type {
            "rust" => self.calculate_metrics(
                &self.rust_latencies,
                &self.rust_requests,
                &self.rust_errors,
            ),
            "legacy" => self.calculate_metrics(
                &self.legacy_latencies,
                &self.legacy_requests,
                &self.legacy_errors,
            ),
            _ => None,
        }
    }

    fn calculate_metrics(
        &self,
        latencies: &Arc<Mutex<Vec<f64>>>,
        requests: &Arc<Mutex<u64>>,
        errors: &Arc<Mutex<u64>>,
    ) -> Option<PerformanceMetrics> {
        let latencies = latencies.lock().ok()?;
        let request_count = *requests.lock().ok()?;
        let error_count = *errors.lock().ok()?;

        if latencies.is_empty() {
            return None;
        }

        let mut sorted_latencies = latencies.clone();
        sorted_latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let len = sorted_latencies.len();
        let p99_latency_ms = sorted_latencies[((len as f64) * 0.99) as usize];
        let p95_latency_ms = sorted_latencies[((len as f64) * 0.95) as usize];
        let p50_latency_ms = sorted_latencies[len / 2];
        let avg_latency_ms = sorted_latencies.iter().sum::<f64>() / len as f64;

        let error_rate = if request_count > 0 {
            (error_count as f64 / request_count as f64) * 100.0
        } else {
            0.0
        };

        // Get system metrics
        let (cpu_usage_percent, memory_usage_mb) = get_system_metrics();

        Some(PerformanceMetrics {
            p99_latency_ms,
            p95_latency_ms,
            p50_latency_ms,
            avg_latency_ms,
            request_count,
            error_rate,
            cpu_usage_percent,
            memory_usage_mb,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        })
    }

    pub fn set_baseline(&self, rust_metrics: PerformanceMetrics, legacy_metrics: PerformanceMetrics) {
        let improvement_factor = if legacy_metrics.p99_latency_ms > 0.0 {
            legacy_metrics.p99_latency_ms / rust_metrics.p99_latency_ms
        } else {
            1.0
        };

        info!(
            rust_p99 = rust_metrics.p99_latency_ms,
            legacy_p99 = legacy_metrics.p99_latency_ms,
            improvement_factor = improvement_factor,
            "Performance baseline established"
        );

        let baseline = PerformanceBaseline {
            rust_metrics,
            legacy_metrics,
            improvement_factor,
        };

        if let Ok(mut baseline_lock) = self.baseline.lock() {
            *baseline_lock = Some(baseline);
        }
    }

    pub fn get_baseline(&self) -> Option<PerformanceBaseline> {
        self.baseline.lock().ok()?.clone()
    }

    pub fn validate_performance(&self) -> PerformanceValidation {
        let rust_metrics = self.get_current_metrics("rust");
        let legacy_metrics = self.get_current_metrics("legacy");
        let baseline = self.get_baseline();

        match (rust_metrics, legacy_metrics, baseline) {
            (Some(rust), Some(legacy), Some(baseline)) => {
                let latency_improvement = if rust.p99_latency_ms > 0.0 {
                    ((baseline.legacy_metrics.p99_latency_ms - rust.p99_latency_ms) 
                     / baseline.legacy_metrics.p99_latency_ms) * 100.0
                } else {
                    0.0
                };

                let memory_improvement = if rust.memory_usage_mb > 0.0 {
                    ((baseline.legacy_metrics.memory_usage_mb - rust.memory_usage_mb) 
                     / baseline.legacy_metrics.memory_usage_mb) * 100.0
                } else {
                    0.0
                };

                let cpu_improvement = if rust.cpu_usage_percent > 0.0 {
                    ((baseline.legacy_metrics.cpu_usage_percent - rust.cpu_usage_percent) 
                     / baseline.legacy_metrics.cpu_usage_percent) * 100.0
                } else {
                    0.0
                };

                PerformanceValidation {
                    latency_improvement_percent: latency_improvement,
                    memory_improvement_percent: memory_improvement,
                    cpu_improvement_percent: cpu_improvement,
                    error_rate_rust: rust.error_rate,
                    error_rate_legacy: legacy.error_rate,
                    meets_latency_target: latency_improvement >= 50.0,
                    meets_resource_target: memory_improvement >= 70.0 || cpu_improvement >= 70.0,
                    meets_error_target: rust.error_rate <= 0.5,
                    overall_success: latency_improvement >= 50.0 
                        && (memory_improvement >= 70.0 || cpu_improvement >= 70.0) 
                        && rust.error_rate <= 0.5,
                }
            }
            _ => PerformanceValidation::default(),
        }
    }

    pub async fn start_monitoring(&self, interval_seconds: u64) {
        let mut interval = interval(Duration::from_secs(interval_seconds));
        
        loop {
            interval.tick().await;
            
            let validation = self.validate_performance();
            
            info!(
                latency_improvement = validation.latency_improvement_percent,
                memory_improvement = validation.memory_improvement_percent,
                cpu_improvement = validation.cpu_improvement_percent,
                rust_error_rate = validation.error_rate_rust,
                legacy_error_rate = validation.error_rate_legacy,
                overall_success = validation.overall_success,
                "Performance validation update"
            );

            if !validation.overall_success {
                warn!(
                    "Performance targets not met - consider rollback",
                );
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceValidation {
    pub latency_improvement_percent: f64,
    pub memory_improvement_percent: f64,
    pub cpu_improvement_percent: f64,
    pub error_rate_rust: f64,
    pub error_rate_legacy: f64,
    pub meets_latency_target: bool,
    pub meets_resource_target: bool,
    pub meets_error_target: bool,
    pub overall_success: bool,
}

impl Default for PerformanceValidation {
    fn default() -> Self {
        Self {
            latency_improvement_percent: 0.0,
            memory_improvement_percent: 0.0,
            cpu_improvement_percent: 0.0,
            error_rate_rust: 0.0,
            error_rate_legacy: 0.0,
            meets_latency_target: false,
            meets_resource_target: false,
            meets_error_target: true,
            overall_success: false,
        }
    }
}

fn get_system_metrics() -> (f64, f64) {
    // Simplified system metrics - in production, use proper system monitoring
    // This is a placeholder implementation
    let cpu_usage = 15.0; // Mock 15% CPU usage for Rust gateway
    let memory_usage = 128.0; // Mock 128MB memory usage for Rust gateway
    
    (cpu_usage, memory_usage)
}

