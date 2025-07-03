use anyhow::Result;
use axum::{
    http::StatusCode,
    middleware::from_fn_with_state,
    response::Json,
    routing::{get, post},
    Router,
};
use metrics_exporter_prometheus::PrometheusBuilder;
use serde_json::{json, Value};
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower_http::{
    compression::CompressionLayer,
    cors::CorsLayer,
    timeout::TimeoutLayer,
    trace::TraceLayer,
};
use tracing::{info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod config;
mod gatekeeper;
mod metrics;
mod middleware;
mod monitoring;
mod routes;

use config::{watcher::ConfigWatcher, AppConfig};

#[derive(Clone)]
pub struct AppState {
    config_watcher: Arc<ConfigWatcher>,
    performance_monitor: Arc<monitoring::PerformanceMonitor>,
}

async fn mirror_test_handler() -> Json<Value> {
    Json(json!({
        "message": "Mirror test endpoint",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "note": "This request should be mirrored if mirror mode is enabled"
    }))
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
    
    // Create performance monitor
    let performance_monitor = Arc::new(monitoring::PerformanceMonitor::new());
    
    // Create application state
    let state = AppState { 
        config_watcher: config_watcher.clone(),
        performance_monitor: performance_monitor.clone(),
    };
    
    // Start config reload monitoring task
    let config_watcher_clone = config_watcher.clone();
    tokio::spawn(async move {
        let mut reload_rx = config_watcher_clone.subscribe_to_reloads();
        while let Ok(_new_config) = reload_rx.recv().await {
            info!("Configuration reloaded: server will use new settings for new connections");
        }
    });
    
    // Start performance monitoring task
    let performance_monitor_clone = performance_monitor.clone();
    tokio::spawn(async move {
        performance_monitor_clone.start_monitoring(60).await; // Monitor every 60 seconds
    });
    
    // Start gatekeeper monitoring task
    let gatekeeper = gatekeeper::Gatekeeper::new(state.clone());
    tokio::spawn(async move {
        gatekeeper.start_monitoring(30).await; // Check every 30 seconds
    });
    
    // Build the application router
    let app = create_app(state).await?;
    
    // Start metrics server
    let metrics_handle = tokio::spawn(async move {
        let metrics_addr = SocketAddr::from(([0, 0, 0, 0], 9090));
        let metrics_app = Router::new()
            .route("/metrics", get(metrics_handler));
        
        let listener = TcpListener::bind(metrics_addr).await.unwrap();
        info!("📊 Metrics server listening on {}", metrics_addr);
        axum::serve(listener, metrics_app).await.unwrap();
    });
    
    // Start the main server
    let addr = format!("{}:{}", initial_config.server.host, initial_config.server.port);
    let listener = TcpListener::bind(&addr).await?;
    
    info!("🚀 Project Gateway starting on {}", addr);
    info!("🔄 Hot-reloading enabled for configuration file: {}", config_path);
    
    // Start main server with graceful shutdown
    let server_handle = tokio::spawn(async move {
        axum::serve(listener, app.into_service())
            .with_graceful_shutdown(shutdown_signal())
            .await
            .unwrap();
    });
    
    // Wait for both servers
    tokio::try_join!(server_handle, metrics_handle)?;
    
    Ok(())
}

async fn create_app(state: AppState) -> Result<Router<AppState>> {
    let current_config = state.config_watcher.get_config().await;
    
    let mut app = Router::new()
        // Health check endpoint
        .route("/health", get(health_check))
        
        // API routes
        .route("/api/v1/health", get(routes::health::health))
        .route("/api/v1/users", get(routes::users::list_users))
        .route("/api/v1/users", post(routes::users::create_user))
        
        // Mirror test endpoint
        .route("/mirror/test", get(mirror_test_handler))
        
        // Gatekeeper status endpoint
        .route("/gatekeeper/status", get(gatekeeper_status_handler));

    // Add canary routing middleware if enabled
    if current_config.canary_rollout.enabled {
        info!("🎯 Canary rollout enabled - {}% traffic to Rust gateway", current_config.canary_rollout.rollout_percentage);
        app = app.layer(from_fn_with_state(state.clone(), middleware::canary::canary_routing_middleware));
    }

    // Add mirror middleware if enabled
    if current_config.mirror.enabled {
        info!("🚨 Mirror mode enabled - target: {}", current_config.mirror.base_url);
        app = app.layer(from_fn_with_state(state.clone(), middleware::mirror::mirror_middleware));
    }

    // Add other middleware layers
    app = app
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(CorsLayer::permissive())
                .layer(CompressionLayer::new())
                .layer(TimeoutLayer::new(Duration::from_secs(
                    current_config.server.timeout_seconds
                )))
        )
        .with_state(state);
    
    Ok(app)
}

async fn gatekeeper_status_handler(State(state): State<AppState>) -> Json<Value> {
    let gatekeeper = gatekeeper::Gatekeeper::new(state);
    let status = gatekeeper.get_status().await;
    
    Json(json!({
        "gatekeeper_status": status,
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "service": "project-gateway"
    }))
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
            "tracing": true,
            "mirror_mode": true
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

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to install CTRL+C signal handler");
}