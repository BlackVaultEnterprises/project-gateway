use axum::{extract::State, http::StatusCode, response::Json};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::{info, warn};

use crate::AppState;

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateUserRequest {
    pub name: String,
    pub email: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub name: String,
    pub email: String,
    pub created_at: String,
}

pub async fn list_users(State(_state): State<AppState>) -> Json<Value> {
    info!("List users requested");

    // TODO: Proxy to upstream service
    // For now, return mock data

    let mock_users = vec![
        User {
            id: uuid::Uuid::new_v4().to_string(),
            name: "John Doe".to_string(),
            email: "john@example.com".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
        },
        User {
            id: uuid::Uuid::new_v4().to_string(),
            name: "Jane Smith".to_string(),
            email: "jane@example.com".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
        },
    ];

    Json(json!({
        "users": mock_users,
        "total": mock_users.len(),
        "note": "Mock data - upstream proxy not yet implemented"
    }))
}

pub async fn create_user(
    State(_state): State<AppState>,
    Json(payload): Json<CreateUserRequest>,
) -> Result<Json<Value>, StatusCode> {
    info!("Create user requested: {:?}", payload);

    // TODO: Proxy to upstream service
    // For now, return mock response

    if payload.name.is_empty() || payload.email.is_empty() {
        warn!("Invalid user creation request: missing name or email");
        return Err(StatusCode::BAD_REQUEST);
    }

    let new_user = User {
        id: uuid::Uuid::new_v4().to_string(),
        name: payload.name,
        email: payload.email,
        created_at: chrono::Utc::now().to_rfc3339(),
    };

    Ok(Json(json!({
        "user": new_user,
        "message": "User created successfully",
        "note": "Mock data - upstream proxy not yet implemented"
    })))
}
