use axum::{extract::State, response::Json};
use serde_json::{json, Value};
use tracing::info;

use crate::AppState;

pub async fn health(State(_state): State<AppState>) -> Json<Value> {
    info!("Health check requested");

    // TODO: Add actual health checks for upstream services
    // For now, return basic health status

    Json(json!({
        "status": "healthy",
        "service": "project-gateway",
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "config_loaded": true,
        "upstream_services": {
            "status": "checking",
            "note": "Upstream health checks not yet implemented"
        }
    }))
}
