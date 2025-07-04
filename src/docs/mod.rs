use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;
use axum::Router;

use crate::{
    routes::{health, users},
    AppState,
};

#[derive(OpenApi)]
#[openapi(
    paths(
        health::health,
        health::health_detailed,
        users::list_users,
        users::create_user,
    ),
    components(
        schemas(
            health::HealthResponse,
            health::DetailedHealthResponse,
            health::ServerConfigInfo,
            health::UpstreamStatus,
            users::User,
            users::CreateUserRequest,
            users::CreateUserResponse,
            users::UserListResponse,
            crate::gatekeeper::GatekeeperStatus,
        )
    ),
    tags(
        (name = "health", description = "Health check endpoints"),
        (name = "users", description = "User management endpoints"),
        (name = "monitoring", description = "Monitoring and status endpoints"),
        (name = "testing", description = "Testing and validation endpoints"),
    ),
    info(
        title = "Project Gateway API",
        version = "0.4.0-alpha",
        description = "High-performance Rust API Gateway - Production-grade internal platform",
        contact(
            name = "BlackVault Enterprises",
            email = "blackvaultenterprises@gmail.com",
        ),
        license(
            name = "MIT",
            url = "https://opensource.org/licenses/MIT",
        ),
    ),
    servers(
        (url = "http://localhost:3000", description = "Local development server"),
        (url = "https://api.gateway.internal", description = "Production server"),
    ),
)]
pub struct ApiDoc;

pub fn create_swagger_router() -> Router<AppState> {
    SwaggerUi::new("/docs")
        .url("/api-docs/openapi.json", ApiDoc::openapi())
        .into()
}

pub fn get_openapi_spec() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}

