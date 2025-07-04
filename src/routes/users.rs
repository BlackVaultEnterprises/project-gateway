use axum::{extract::State, http::StatusCode, response::Json};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::AppState;

#[derive(Serialize, Deserialize, ToSchema)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub created_at: String,
    pub active: bool,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct CreateUserRequest {
    pub username: String,
    pub email: String,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct CreateUserResponse {
    pub user: User,
    pub message: String,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct UserListResponse {
    pub users: Vec<User>,
    pub total: usize,
    pub page: usize,
    pub per_page: usize,
}

/// List all users
///
/// Returns a paginated list of all users in the system.
#[utoipa::path(
    get,
    path = "/api/v1/users",
    tag = "users",
    responses(
        (status = 200, description = "List of users retrieved successfully", body = UserListResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden - insufficient permissions")
    )
)]
pub async fn list_users(State(_state): State<AppState>) -> Json<UserListResponse> {
    // Mock data for demonstration
    let mock_users = vec![
        User {
            id: Uuid::new_v4(),
            username: "admin".to_string(),
            email: "admin@gateway.internal".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            active: true,
        },
        User {
            id: Uuid::new_v4(),
            username: "developer".to_string(),
            email: "dev@gateway.internal".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            active: true,
        },
    ];

    Json(UserListResponse {
        total: mock_users.len(),
        page: 1,
        per_page: 10,
        users: mock_users,
    })
}

/// Create a new user
///
/// Creates a new user with the provided username and email.
#[utoipa::path(
    post,
    path = "/api/v1/users",
    tag = "users",
    request_body = CreateUserRequest,
    responses(
        (status = 201, description = "User created successfully", body = CreateUserResponse),
        (status = 400, description = "Invalid request data"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden - insufficient permissions"),
        (status = 409, description = "User already exists")
    )
)]
pub async fn create_user(
    State(_state): State<AppState>,
    Json(payload): Json<CreateUserRequest>,
) -> Result<Json<CreateUserResponse>, StatusCode> {
    // Basic validation
    if payload.username.is_empty() || payload.email.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Create mock user
    let new_user = User {
        id: Uuid::new_v4(),
        username: payload.username,
        email: payload.email,
        created_at: chrono::Utc::now().to_rfc3339(),
        active: true,
    };

    Ok(Json(CreateUserResponse {
        user: new_user,
        message: "User created successfully".to_string(),
    }))
}

