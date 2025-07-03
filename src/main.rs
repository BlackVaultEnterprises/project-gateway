use anyhow::Result;
use axum::{
    extract::State,
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

use config::AppConfig;

#[derive(Clone)]
pub struct AppState {
    config: Arc<AppConfig>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables
    dotenvy::dotenv().ok();
    
    // Initialize tracing
    init_tracing()?;
    
    // Load configuration
    let config = Arc::new(AppConfig::load()?);
    info!("Configuration loaded successfully");
    
    // Initialize metrics
    init_metrics(&config)?;
    
    // Create application state
    let state = AppState { config: config.clone() };
    
    // Build the application router
    let app = create_app(state).await?;
    
    // Start the server
    let addr = format!("{}:{}", config.server.host, config.server.port);
    let listener = TcpListener::bind(&addr).await?;
    
    info!("🚀 Project Gateway starting on {}", addr);
    info!("📊 Metrics available at http://{}:{}/metrics", 
          config.server.host, config.metrics.port);
    
    axum::serve(listener, app).await?;
    
    Ok(())
}

async fn create_app(state: AppState) -> Result<Router> {
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
                    state.config.server.timeout_seconds
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
        "timestamp": chrono::Utc::now().to_rfc3339()
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

