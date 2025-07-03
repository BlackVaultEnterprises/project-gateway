use std::sync::Arc;

pub mod config;
pub mod gatekeeper;
pub mod metrics;
pub mod middleware;
pub mod monitoring;
pub mod routes;

#[derive(Clone)]
pub struct AppState {
    pub config_watcher: Arc<config::watcher::ConfigWatcher>,
    pub performance_monitor: Arc<monitoring::PerformanceMonitor>,
}
