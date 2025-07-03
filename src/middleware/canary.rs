use axum::{
    body::Body,
    extract::State,
    http::{Request, Response, HeaderMap, StatusCode},
    middleware::Next,
    response::Json,
};
use serde_json::{json, Value};
use std::{sync::Arc, time::Instant};
use tokio::time::timeout;
use tracing::{info, warn, error};

use crate::{config::CanaryRolloutConfig, AppState};

pub async fn canary_routing_middleware(
    State(state): State<AppState>,
    request: Request<Body>,
    next: Next,
) -> Response<Body> {
    let start_time = Instant::now();
    let config = state.config_watcher.get_config().await;
    
    if !config.canary_rollout.enabled {
        return next.run(request).await;
    }

    // Check for header-based routing override
    let headers = request.headers();
    let force_rust = headers.get(&config.canary_rollout.trigger_header)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.to_lowercase() == "rust")
        .unwrap_or(false);

    let force_legacy = headers.get(&config.canary_rollout.trigger_header)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.to_lowercase() == "legacy")
        .unwrap_or(false);

    // Determine routing decision
    let use_rust_gateway = if force_rust {
        info!("Header override: routing to Rust gateway");
        true
    } else if force_legacy {
        info!("Header override: routing to legacy gateway");
        false
    } else {
        // Use rollout percentage for automatic canary routing
        let random_value: f64 = rand::random();
        let should_use_rust = random_value * 100.0 < config.canary_rollout.rollout_percentage;
        
        if should_use_rust {
            info!(
                rollout_percentage = config.canary_rollout.rollout_percentage,
                random_value = random_value * 100.0,
                "Canary routing: using Rust gateway"
            );
        }
        
        should_use_rust
    };

    if use_rust_gateway {
        // Route to Rust gateway (current implementation)
        let response = next.run(request).await;
        let latency = start_time.elapsed();
        
        // Record metrics for Rust gateway
        let latency_ms = latency.as_millis() as f64;
        let is_error = response.status().is_server_error();
        
        state.performance_monitor.record_request("rust", latency_ms, is_error);
        
        crate::metrics::record_gateway_request(
            "rust",
            response.status().as_u16(),
            latency.as_secs_f64()
        );
        
        response
    } else {
        // Route to legacy gateway
        route_to_legacy_gateway(request, &config.canary_rollout, start_time, &state).await
    }
}

async fn route_to_legacy_gateway(
    request: Request<Body>,
    config: &CanaryRolloutConfig,
    start_time: Instant,
    state: &AppState,
) -> Response<Body> {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let headers = request.headers().clone();
    
    // Construct legacy gateway URL
    let legacy_url = format!("{}{}", config.legacy_gateway_url, uri.path_and_query().map(|pq| pq.as_str()).unwrap_or(""));
    
    let client = reqwest::Client::new();
    
    // Prepare request to legacy gateway
    let mut legacy_request = client.request(method.clone(), &legacy_url);
    
    // Copy headers (excluding hop-by-hop headers)
    for (key, value) in headers.iter() {
        if key != "host" && key != "connection" && key != "upgrade" {
            legacy_request = legacy_request.header(key, value);
        }
    }
    
    // Add routing header to identify source
    legacy_request = legacy_request.header("X-Routed-By", "Rust-Gateway-Canary");
    
    match timeout(
        std::time::Duration::from_secs(30),
        legacy_request.send(),
    ).await {
        Ok(Ok(legacy_response)) => {
            let latency = start_time.elapsed();
            let status = legacy_response.status();
            
            info!(
                method = %method,
                path = uri.path(),
                status = status.as_u16(),
                latency_ms = latency.as_millis(),
                "Legacy gateway response"
            );
            
            // Record metrics for legacy gateway
            let latency_ms = latency.as_millis() as f64;
            let is_error = status.is_server_error();
            
            state.performance_monitor.record_request("legacy", latency_ms, is_error);
            
            crate::metrics::record_gateway_request(
                "legacy",
                status.as_u16(),
                latency.as_secs_f64()
            );
            
            // Convert reqwest response to axum response
            let mut response_builder = Response::builder().status(status);
            
            // Copy response headers
            for (key, value) in legacy_response.headers() {
                response_builder = response_builder.header(key, value);
            }
            
            // Get response body
            match legacy_response.bytes().await {
                Ok(body_bytes) => {
                    response_builder
                        .body(Body::from(body_bytes))
                        .unwrap_or_else(|_| {
                            Response::builder()
                                .status(StatusCode::INTERNAL_SERVER_ERROR)
                                .body(Body::from("Failed to build response"))
                                .unwrap()
                        })
                }
                Err(e) => {
                    error!("Failed to read legacy gateway response body: {}", e);
                    crate::metrics::record_gateway_request("legacy", 500, latency.as_secs_f64());
                    
                    Response::builder()
                        .status(StatusCode::BAD_GATEWAY)
                        .header("content-type", "application/json")
                        .body(Body::from(json!({
                            "error": "Legacy gateway response error",
                            "message": "Failed to read response body"
                        }).to_string()))
                        .unwrap()
                }
            }
        }
        Ok(Err(e)) => {
            let latency = start_time.elapsed();
            error!("Legacy gateway request failed: {}", e);
            
            let latency_ms = latency.as_millis() as f64;
            state.performance_monitor.record_request("legacy", latency_ms, true);
            crate::metrics::record_gateway_request("legacy", 502, latency.as_secs_f64());
            
            Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .header("content-type", "application/json")
                .body(Body::from(json!({
                    "error": "Legacy gateway unavailable",
                    "message": format!("Request failed: {}", e)
                }).to_string()))
                .unwrap()
        }
        Err(_) => {
            let latency = start_time.elapsed();
            error!("Legacy gateway request timeout");
            
            let latency_ms = latency.as_millis() as f64;
            state.performance_monitor.record_request("legacy", latency_ms, true);
            crate::metrics::record_gateway_request("legacy", 504, latency.as_secs_f64());
            
            Response::builder()
                .status(StatusCode::GATEWAY_TIMEOUT)
                .header("content-type", "application/json")
                .body(Body::from(json!({
                    "error": "Legacy gateway timeout",
                    "message": "Request timed out after 30 seconds"
                }).to_string()))
                .unwrap()
        }
    }
}

