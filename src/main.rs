use anyhow::Result;
use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde_json::{json, Value};
use std::{
    net::SocketAddr,
    sync::Arc,
    time::Duration,
};
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower_http::{
    cors::CorsLayer,
    compression::CompressionLayer,
    timeout::TimeoutLayer,
    trace::TraceLayer,
};
use tracing::{info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utoipa::ToSchema;

mod config;
mod docs;
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

#[derive(serde::Serialize, ToSchema)]
pub struct MirrorTestResponse {
    pub message: String,
    pub timestamp: String,
    pub note: String,
}

/// Mirror test endpoint
///
/// Test endpoint for validating mirror functionality.
#[utoipa::path(
    get,
    path = "/mirror/test",
    tag = "testing",
    responses(
        (status = 200, description = "Mirror test response", body = MirrorTestResponse)
    )
)]
async fn mirror_test_handler() -> Json<MirrorTestResponse> {
    Json(MirrorTestResponse {
        message: "Mirror test endpoint".to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        note: "This request should be mirrored if mirror mode is enabled".to_string(),
    })
}

/// Gatekeeper status endpoint
///
/// Returns the current status of the gatekeeper and rollout system.
#[utoipa::path(
    get,
    path = "/gatekeeper/status",
    tag = "monitoring",
    responses(
        (status = 200, description = "Gatekeeper status", body = gatekeeper::GatekeeperStatus)
    )
)]
async fn gatekeeper_status_handler(State(state): State<AppState>) -> Json<gatekeeper::GatekeeperStatus> {
    // Mock gatekeeper status for now
    Json(gatekeeper::GatekeeperStatus {
        is_healthy: true,
        current_rollout_percentage: 100.0,
        error_rate: 0.1,
        latency_degradation_percent: 0.0,
        last_check: chrono::Utc::now().timestamp() as u64,
        rollback_triggered: false,
        rollback_reason: None,
    })
}

async fn health_check() -> Json<Value> {
    Json(json!({
        "status": "healthy",
        "service": "project-gateway",
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

async fn create_app(state: AppState) -> Result<Router> {
    let mut app = Router::new()
        // Health endpoints
        .route("/health", get(routes::health::health))
        .route("/api/v1/health", get(routes::health::health_detailed))
        
        // User management endpoints
        .route("/api/v1/users", get(routes::users::list_users))
        .route("/api/v1/users", post(routes::users::create_user))
        
        // Monitoring endpoints
        .route("/gatekeeper/status", get(gatekeeper_status_handler))
        
        // Testing endpoints
        .route("/mirror/test", get(mirror_test_handler))
        
        // Swagger UI and OpenAPI documentation
        .merge(docs::create_swagger_router())
        .route("/api-docs/openapi.json", get(|| async {
            Json(docs::get_openapi_spec())
        }));

    // Add middleware stack
    app = app.layer(
        ServiceBuilder::new()
            .layer(TraceLayer::new_for_http())
            .layer(CorsLayer::permissive())
            .layer(CompressionLayer::new())
            .layer(TimeoutLayer::new(Duration::from_secs(30)))
    );

    // Add canary routing middleware if enabled
    let config = state.config_watcher.get_config().await;
    if config.canary_rollout.enabled {
        app = app.layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::canary::canary_routing_middleware,
        ));
    }

    // Add mirror middleware if enabled
    if config.mirror.enabled {
        app = app.layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::mirror::mirror_middleware,
        ));
    }

    app = app.with_state(state);
    
    Ok(app)
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "project_gateway=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer().json())
        .init();

    info!("🚀 Starting Project Gateway v{}", env!("CARGO_PKG_VERSION"));

    // Load environment variables
    dotenvy::dotenv().ok();

    // Create configuration watcher
    let config_watcher = Arc::new(ConfigWatcher::new("config/default.yaml").await?);
    
    // Create performance monitor
    let performance_monitor = Arc::new(monitoring::PerformanceMonitor::new());

    // Create application state
    let state = AppState {
        config_watcher: config_watcher.clone(),
        performance_monitor: performance_monitor.clone(),
    };

    // Start config reload monitoring
    let config_watcher_clone = config_watcher.clone();
    tokio::spawn(async move {
        config_watcher_clone.start_watching().await;
    });

    // Start performance monitoring task
    let performance_monitor_clone = performance_monitor.clone();
    tokio::spawn(async move {
        performance_monitor_clone.start_monitoring(60).await; // 60 second intervals
    });

    // Start gatekeeper monitoring
    let gatekeeper = Arc::new(gatekeeper::Gatekeeper::new(state.clone()));
    let gatekeeper_clone = gatekeeper.clone();
    tokio::spawn(async move {
        gatekeeper_clone.start_monitoring(30).await; // 30 second intervals
    });

    // Create the application
    let app = create_app(state).await?;

    // Get server configuration
    let config = config_watcher.get_config().await;
    let addr = SocketAddr::from(([0, 0, 0, 0], config.server.port));

    info!("🌐 Server listening on http://{}", addr);
    info!("📚 API Documentation available at http://{}/docs", addr);
    info!("📊 Metrics available at http://{}:{}/metrics", 
          config.server.host, config.server.metrics_port);

    // Start main server with graceful shutdown
    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

