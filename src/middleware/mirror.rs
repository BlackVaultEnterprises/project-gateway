use axum::{
    body::Body,
    extract::State,
    http::{Request, Response, HeaderValue},
    middleware::Next,
};
use std::sync::Arc;
use tokio::time::Instant;
use tracing::{error, info};

use crate::{config::watcher::ConfigWatcher, metrics::MIRROR_METRICS, AppState};

pub async fn mirror_middleware(
    State(state): State<AppState>,
    request: Request<Body>,
    next: Next,
) -> Response<Body> {
    let start = Instant::now();
    let current_config = state.config_watcher.get_config().await;
    
    if !current_config.mirror.enabled {
        return next.run(request).await;
    }

    // Clone request data for mirroring
    let method = request.method().clone();
    let uri = request.uri().clone();
    let headers = request.headers().clone();
    
    // Process main request first
    let response = next.run(request).await;
    let main_latency = start.elapsed();
    
    // Fire and forget mirror request
    let mirror_url = format!("{}{}", current_config.mirror.base_url, uri.path_and_query().map(|pq| pq.as_str()).unwrap_or(""));
    let client = reqwest::Client::new();
    
    tokio::spawn(async move {
        let mirror_start = Instant::now();
        
        let mut mirror_request = client.request(method.clone(), &mirror_url);
        
        // Copy headers
        for (key, value) in headers.iter() {
            if key != "host" {
                mirror_request = mirror_request.header(key, value);
            }
        }
        
        // Add mirror header
        mirror_request = mirror_request.header("X-Mirrored-By", "Rust-Gateway");
        
        // Send mirror request
        match mirror_request.send().await {
            Ok(mirror_response) => {
                let mirror_latency = mirror_start.elapsed();
                let status = mirror_response.status().as_u16() as i32;
                
                // Record metrics
                MIRROR_METRICS.requests_total.increment(1);
                MIRROR_METRICS.latency_seconds.record(mirror_latency.as_secs_f64());
                
                // Log the mirror result
                info!(
                    path = uri.path(),
                    mirror_status = status,
                    mirror_latency_ms = mirror_latency.as_millis(),
                    main_latency_ms = main_latency.as_millis(),
                    latency_delta_ms = mirror_latency.as_millis() as i64 - main_latency.as_millis() as i64,
                    "Mirror request completed"
                );
            }
            Err(e) => {
                MIRROR_METRICS.failures_total.increment(1);
                error!(
                    path = uri.path(),
                    error = %e,
                    "Mirror request failed"
                );
            }
        }
    });
    
    response
}

