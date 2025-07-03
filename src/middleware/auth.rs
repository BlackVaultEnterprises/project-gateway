// TODO: Implement JWT authentication middleware
// This will be implemented in Phase 1 completion

use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::Response,
};

#[allow(dead_code)]
pub async fn auth_middleware(request: Request, next: Next) -> Result<Response, StatusCode> {
    // TODO: Implement JWT validation
    // For now, pass through all requests
    Ok(next.run(request).await)
}