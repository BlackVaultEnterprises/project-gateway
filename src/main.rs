use anyhow::Result;
use axum::{
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use metrics_exporter_prometheus::PrometheusBuilder;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower_http::{
    cors::CorsLayer,
    trace::TraceLayer,
    timeout::TimeoutLayer,
    compression::CompressionLayer,
};
use tracing::{info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod config;
mod middleware;
mod routes;

use config::{watcher::ConfigWatcher, AppConfig};

#[derive(Clone)]
pub struct AppState {
    config_watcher: Arc<ConfigWatcher>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables
    dotenvy::dotenv().ok();
    
    // Initialize tracing
    init_tracing()?;
    
    // Load initial configuration
    let initial_config = AppConfig::load()?;
    info!("Initial configuration loaded successfully");
    
    // Initialize metrics
    init_metrics(&initial_config)?;
    
    // Create configuration watcher
    let config_path = std::env::var("CONFIG_PATH")
        .unwrap_or_else(|_| "config/default.yaml".to_string());
    let config_watcher = Arc::new(ConfigWatcher::new(&config_path, initial_config.clone())?);
    
    // Create application state
    let state = AppState { 
        config_watcher: config_watcher.clone() 
    };
    
    // Start config reload monitoring task
    let config_watcher_clone = config_watcher.clone();
    tokio::spawn(async move {
        let mut reload_rx = config_watcher_clone.subscribe_to_reloads();
        while let Ok(new_config) = reload_rx.recv().await {
            info!("Configuration reloaded: server will use new settings for new connections");
            // Here you could implement graceful reconfiguration of services
            // For now, we just log the reload event
        }
    });
    
    // Build the application router
    let app = create_app(state).await?;
    
    // Start the server
    let addr = format!("{}:{}", initial_config.server.host, initial_config.server.port);
    let listener = TcpListener::bind(&addr).await?;
    
    info!("🚀 Project Gateway starting on {}", addr);
    info!("📊 Metrics available at http://{}:{}/metrics", 
          initial_config.server.host, initial_config.metrics.port);
    info!("🔄 Hot-reloading enabled for configuration file: {}", config_path);
    
    axum::serve(listener, app).await?;
    
    Ok(())
}

async fn create_app(state: AppState) -> Result<Router> {
    let current_config = state.config_watcher.get_config().await;
    
    let app = Router::new()
        // Health check endpoint
        .route("/health", get(health_check))
        
        // API routes
        .route("/api/v1/health", get(routes::health::health))
        .route("/api/v1/users", get(routes::users::list_users))
        .route("/api/v1/users", post(routes::users::create_user))
        
        // Metrics endpoint
        .route("/metrics", get(metrics_handler))
        
        // Add middleware layers
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(CorsLayer::permissive())
                .layer(CompressionLayer::new())
                .layer(TimeoutLayer::new(std::time::Duration::from_secs(
                    current_config.server.timeout_seconds
                )))
        )
        .with_state(state);
    
    Ok(app)
}

async fn health_check() -> Json<Value> {
    Json(json!({
        "status": "healthy",
        "service": "project-gateway",
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "features": {
            "hot_reload": true,
            "metrics": true,
            "tracing": true
        }
    }))
}

async fn metrics_handler() -> Result<String, StatusCode> {
    let encoder = prometheus::TextEncoder::new();
    let metric_families = prometheus::gather();
    
    encoder
        .encode_to_string(&metric_families)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

fn init_tracing() -> Result<()> {
    let log_format = std::env::var("LOG_FORMAT").unwrap_or_else(|_| "json".to_string());
    
    let subscriber = tracing_subscriber::registry();
    
    match log_format.as_str() {
        "json" => {
            subscriber
                .with(tracing_subscriber::fmt::layer().json())
                .with(tracing_subscriber::EnvFilter::from_default_env())
                .init();
        }
        _ => {
            subscriber
                .with(tracing_subscriber::fmt::layer())
                .with(tracing_subscriber::EnvFilter::from_default_env())
                .init();
        }
    }
    
    Ok(())
}

fn init_metrics(config: &AppConfig) -> Result<()> {
    if config.metrics.enabled {
        let builder = PrometheusBuilder::new();
        builder
            .with_http_listener(([0, 0, 0, 0], config.metrics.port))
            .install()?;
        
        info!("Metrics exporter initialized on port {}", config.metrics.port);
    }
    
    Ok(())
}