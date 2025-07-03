use std::sync::Arc;

pub mod config;
pub mod middleware;
pub mod routes;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<config::AppConfig>,
}
