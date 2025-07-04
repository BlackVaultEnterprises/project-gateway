use axum::{extract::State, response::Json};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use tracing::info;

use crate::AppState;

#[derive(Serialize, Deserialize, ToSchema)]
pub struct HealthResponse {
    pub status: String,
    pub service: String,
    pub version: String,
    pub timestamp: String,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct DetailedHealthResponse {
    pub status: String,
    pub service: String,
    pub version: String,
    pub timestamp: String,
    pub config_loaded: bool,
    pub hot_reload_enabled: bool,
    pub server_config: ServerConfigInfo,
    pub upstream_services: UpstreamStatus,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct ServerConfigInfo {
    pub host: String,
    pub port: u16,
    pub timeout_seconds: u64,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct UpstreamStatus {
    pub status: String,
    pub note: String,
}

/// Basic health check endpoint
///
/// Returns a simple health status indicating the service is running.
#[utoipa::path(
    get,
    path = "/health",
    tag = "health",
    responses(
        (status = 200, description = "Service is healthy", body = HealthResponse),
        (status = 503, description = "Service is unhealthy")
    )
)]
pub async fn health(State(_state): State<AppState>) -> Json<HealthResponse> {
    info!("Basic health check requested");
    
    Json(HealthResponse {
        status: "healthy".to_string(),
        service: "project-gateway".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
    })
}

/// Detailed health check endpoint
///
/// Returns comprehensive health information including configuration status,
/// system metrics, and operational details.
#[utoipa::path(
    get,
    path = "/api/v1/health",
    tag = "health",
    responses(
        (status = 200, description = "Detailed health information", body = DetailedHealthResponse),
        (status = 503, description = "Service is unhealthy")
    )
)]
pub async fn health_detailed(State(state): State<AppState>) -> Json<DetailedHealthResponse> {
    info!("Detailed health check requested");
    
    let config = state.config_watcher.get_config().await;
    
    Json(DetailedHealthResponse {
        status: "healthy".to_string(),
        service: "project-gateway".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        config_loaded: true,
        hot_reload_enabled: true,
        server_config: ServerConfigInfo {
            host: config.server.host,
            port: config.server.port,
            timeout_seconds: config.server.timeout_seconds,
        },
        upstream_services: UpstreamStatus {
            status: "checking".to_string(),
            note: "Upstream health checks not yet implemented".to_string(),
        },
    })
}

