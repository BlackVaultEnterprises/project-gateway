use std::sync::Arc;

pub mod config;
pub mod metrics;
pub mod middleware;
pub mod routes;

#[derive(Clone)]
pub struct AppState {
    pub config_watcher: Arc<config::watcher::ConfigWatcher>,
}
