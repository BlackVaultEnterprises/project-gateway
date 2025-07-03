use anyhow::Result;
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tracing::{error, info, warn};

use super::AppConfig;

pub struct ConfigWatcher {
    config: Arc<RwLock<AppConfig>>,
    _watcher: RecommendedWatcher,
    reload_tx: broadcast::Sender<AppConfig>,
}

impl ConfigWatcher {
    pub fn new(config_path: &str, initial_config: AppConfig) -> Result<Self> {
        let config = Arc::new(RwLock::new(initial_config));
        let (reload_tx, _) = broadcast::channel(16);
        
        let config_clone = config.clone();
        let reload_tx_clone = reload_tx.clone();
        
        let mut watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                match res {
                    Ok(event) => {
                        if event.kind.is_modify() {
                            info!("Configuration file changed, reloading...");
                            
                            match AppConfig::load() {
                                Ok(new_config) => {
                                    // Use blocking task to handle async operations in sync context
                                    let config_clone = config_clone.clone();
                                    let reload_tx_clone = reload_tx_clone.clone();
                                    let new_config_clone = new_config.clone();
                                    
                                    std::thread::spawn(move || {
                                        let rt = tokio::runtime::Handle::try_current()
                                            .or_else(|_| {
                                                tokio::runtime::Runtime::new()
                                                    .map(|rt| rt.handle().clone())
                                            });
                                        
                                        if let Ok(handle) = rt {
                                            handle.spawn(async move {
                                                let mut config_guard = config_clone.write().await;
                                                *config_guard = new_config_clone.clone();
                                                drop(config_guard);
                                                
                                                if let Err(e) = reload_tx_clone.send(new_config_clone) {
                                                    warn!("No active config reload subscribers: {}", e);
                                                } else {
                                                    info!("Configuration reloaded successfully");
                                                }
                                            });
                                        } else {
                                            error!("Failed to get tokio runtime handle for config reload");
                                        }
                                    });
                                }
                                Err(e) => {
                                    error!("Failed to reload configuration: {}", e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("File watcher error: {}", e);
                    }
                }
            },
            Config::default(),
        )?;
        
        watcher.watch(Path::new(config_path), RecursiveMode::NonRecursive)?;
        info!("Started watching configuration file: {}", config_path);
        
        Ok(ConfigWatcher {
            config,
            _watcher: watcher,
            reload_tx,
        })
    }
    
    pub async fn get_config(&self) -> AppConfig {
        self.config.read().await.clone()
    }
    
    pub fn subscribe_to_reloads(&self) -> broadcast::Receiver<AppConfig> {
        self.reload_tx.subscribe()
    }
}

