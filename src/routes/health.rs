use axum::{extract::State, response::Json};
use serde_json::{json, Value};
use tracing::info;

use crate::AppState;

pub async fn health(State(state): State<AppState>) -> Json<Value> {
    info!("Health check requested");
    
    let config = state.config_watcher.get_config().await;
    
    // TODO: Add actual health checks for upstream services
    // For now, return basic health status
    
    Json(json!({
        "status": "healthy",
        "service": "project-gateway",
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "config_loaded": true,
        "hot_reload_enabled": true,
        "upstream_services": {
            "status": "checking",
            "note": "Upstream health checks not yet implemented"
        },
        "server_config": {
            "host": config.server.host,
            "port": config.server.port,
            "timeout_seconds": config.server.timeout_seconds
        }
    }))
}