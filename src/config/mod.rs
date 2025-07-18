use anyhow::Result;
use serde::{Deserialize, Serialize};

pub mod watcher;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub metrics: MetricsConfig,
    pub tracing: TracingConfig,
    pub mirror: MirrorConfig,
    pub canary_rollout: CanaryRolloutConfig,
    pub routes: Vec<RouteConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanaryRolloutConfig {
    pub enabled: bool,
    pub rollout_percentage: f64,
    pub step: f64,
    pub max_errors: f64,
    pub monitor_latency_p99: bool,
    pub monitor_memory_cpu: bool,
    pub trigger_header: String,
    pub success_window_seconds: u64,
    pub legacy_gateway_url: String,
    pub webhook_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteConfig {
    pub path: String,
    pub method: String,
    pub legacy_endpoint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    pub enabled: bool,
    pub port: u16,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TracingConfig {
    pub enabled: bool,
    pub jaeger_endpoint: String,
    pub service_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MirrorConfig {
    pub enabled: bool,
    pub base_url: String,
    pub timeout_ms: u64,
    pub retry_failed: bool,
    pub max_retries: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiddlewareConfig {
    pub cors: CorsConfig,
    pub rate_limiting: RateLimitingConfig,
    pub auth: AuthConfig,
    pub logging: LoggingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorsConfig {
    pub enabled: bool,
    pub allow_origins: Vec<String>,
    pub allow_methods: Vec<String>,
    pub allow_headers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitingConfig {
    pub enabled: bool,
    pub requests_per_minute: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub enabled: bool,
    pub jwt_secret: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub enabled: bool,
    pub include_request_body: bool,
    pub include_response_body: bool,
}

impl AppConfig {
    pub fn load() -> Result<Self> {
        let config_path = std::env::var("CONFIG_PATH")
            .unwrap_or_else(|_| "config/default.yaml".to_string());
        
        let mut builder = config::Config::builder()
            .add_source(config::File::with_name(&config_path))
            .add_source(config::Environment::with_prefix("GATEWAY"));
        
        // Override with environment variables if present
        if let Ok(host) = std::env::var("HOST") {
            builder = builder.set_override("server.host", host)?;
        }
        if let Ok(port) = std::env::var("PORT") {
            builder = builder.set_override("server.port", port.parse::<u16>()?)?;
        }
        if let Ok(metrics_port) = std::env::var("METRICS_PORT") {
            builder = builder.set_override("metrics.port", metrics_port.parse::<u16>()?)?;
        }
        
        let settings = builder.build()?;
        let config: AppConfig = settings.try_deserialize()?;
        Ok(config)
    }
}