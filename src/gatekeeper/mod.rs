use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use serde::{Deserialize, Serialize};
use tokio::time::interval;
use tracing::{info, warn, error};
use utoipa::ToSchema;

use crate::{
    config::CanaryRolloutConfig,
    monitoring::{PerformanceMonitor, PerformanceValidation},
    AppState,
};

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct GatekeeperStatus {
    pub is_healthy: bool,
    pub current_rollout_percentage: f64,
    pub error_rate: f64,
    pub latency_degradation_percent: f64,
    pub last_check: u64,
    pub rollback_triggered: bool,
    pub rollback_reason: Option<String>,
}

pub struct Gatekeeper {
    state: AppState,
    last_rollback: Arc<Mutex<Option<Instant>>>,
    rollback_cooldown: Duration,
}

impl Gatekeeper {
    pub fn new(state: AppState) -> Self {
        Self {
            state,
            last_rollback: Arc::new(Mutex::new(None)),
            rollback_cooldown: Duration::from_secs(300), // 5 minute cooldown
        }
    }

    pub async fn start_monitoring(&self, check_interval_seconds: u64) {
        let mut interval = interval(Duration::from_secs(check_interval_seconds));
        
        info!("🛡️ Gatekeeper monitoring started - checking every {} seconds", check_interval_seconds);
        
        loop {
            interval.tick().await;
            
            let status = self.check_health().await;
            
            if !status.is_healthy && !status.rollback_triggered {
                warn!(
                    error_rate = status.error_rate,
                    latency_degradation = status.latency_degradation_percent,
                    rollout_percentage = status.current_rollout_percentage,
                    "🚨 Gatekeeper detected degradation - triggering rollback"
                );
                
                if let Some(reason) = &status.rollback_reason {
                    self.trigger_rollback(reason).await;
                }
            } else if status.is_healthy {
                info!(
                    error_rate = status.error_rate,
                    rollout_percentage = status.current_rollout_percentage,
                    "✅ Gatekeeper health check passed"
                );
            }
        }
    }

    async fn check_health(&self) -> GatekeeperStatus {
        let config = self.state.config_watcher.get_config().await;
        let validation = self.state.performance_monitor.validate_performance();
        
        let current_rollout_percentage = config.canary_rollout.rollout_percentage;
        let error_rate = validation.error_rate_rust;
        
        // Check if we're in rollback cooldown
        let in_cooldown = {
            if let Ok(last_rollback) = self.last_rollback.lock() {
                if let Some(last) = *last_rollback {
                    last.elapsed() < self.rollback_cooldown
                } else {
                    false
                }
            } else {
                false
            }
        };

        let mut is_healthy = true;
        let mut rollback_reason = None;

        // Check error rate threshold
        if error_rate > config.canary_rollout.max_errors {
            is_healthy = false;
            rollback_reason = Some(format!(
                "Error rate {}% exceeds threshold {}%",
                error_rate, config.canary_rollout.max_errors
            ));
        }

        // Check latency degradation (if we have baseline)
        let latency_degradation_percent = if validation.latency_improvement_percent < 0.0 {
            validation.latency_improvement_percent.abs()
        } else {
            0.0
        };

        if latency_degradation_percent > 10.0 {
            is_healthy = false;
            rollback_reason = Some(format!(
                "Latency degraded by {}% (threshold: 10%)",
                latency_degradation_percent
            ));
        }

        // Don't trigger rollback if we're in cooldown
        if in_cooldown {
            is_healthy = true;
            rollback_reason = None;
        }

        GatekeeperStatus {
            is_healthy,
            current_rollout_percentage,
            error_rate,
            latency_degradation_percent,
            last_check: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            rollback_triggered: !is_healthy && rollback_reason.is_some(),
            rollback_reason,
        }
    }

    async fn trigger_rollback(&self, reason: &str) {
        error!("🚨 TRIGGERING AUTOMATIC ROLLBACK: {}", reason);
        
        // Update last rollback time
        if let Ok(mut last_rollback) = self.last_rollback.lock() {
            *last_rollback = Some(Instant::now());
        }

        let current_config = self.state.config_watcher.get_config().await;
        let current_percentage = current_config.canary_rollout.rollout_percentage;
        
        // Calculate rollback percentage (reduce by step size, minimum 1%)
        let rollback_percentage = (current_percentage - current_config.canary_rollout.step).max(1.0);
        
        info!(
            "Rolling back from {}% to {}%",
            current_percentage, rollback_percentage
        );

        // Send webhook notification
        self.send_rollback_alert(reason, current_percentage, rollback_percentage).await;
        
        // Update configuration (in a real system, this would update the config file)
        // For now, we'll log the action
        warn!(
            "ROLLBACK EXECUTED: {} -> {}% (reason: {})",
            current_percentage, rollback_percentage, reason
        );
    }

    async fn send_rollback_alert(&self, reason: &str, from_percentage: f64, to_percentage: f64) {
        let config = self.state.config_watcher.get_config().await;
        
        if config.canary_rollout.webhook_url.starts_with("http") {
            let payload = serde_json::json!({
                "text": format!(
                    "🚨 AUTOMATIC ROLLBACK TRIGGERED\n\
                     Reason: {}\n\
                     Rollout: {}% → {}%\n\
                     Time: {}\n\
                     Service: project-gateway",
                    reason,
                    from_percentage,
                    to_percentage,
                    chrono::Utc::now().to_rfc3339()
                ),
                "username": "Gateway Gatekeeper",
                "icon_emoji": ":warning:"
            });

            let client = reqwest::Client::new();
            match client
                .post(&config.canary_rollout.webhook_url)
                .json(&payload)
                .send()
                .await
            {
                Ok(response) => {
                    if response.status().is_success() {
                        info!("Rollback alert sent successfully");
                    } else {
                        warn!("Failed to send rollback alert: {}", response.status());
                    }
                }
                Err(e) => {
                    error!("Error sending rollback alert: {}", e);
                }
            }
        } else {
            info!("Webhook URL not configured, skipping alert");
        }
    }

    pub async fn get_status(&self) -> GatekeeperStatus {
        self.check_health().await
    }

    pub async fn force_rollback(&self, reason: &str) {
        warn!("🔧 MANUAL ROLLBACK TRIGGERED: {}", reason);
        self.trigger_rollback(reason).await;
    }

    pub async fn advance_rollout(&self) {
        let current_config = self.state.config_watcher.get_config().await;
        let current_percentage = current_config.canary_rollout.rollout_percentage;
        let step = current_config.canary_rollout.step;
        
        let new_percentage = (current_percentage + step).min(100.0);
        
        if new_percentage > current_percentage {
            info!(
                "🚀 Advancing rollout: {}% → {}%",
                current_percentage, new_percentage
            );
            
            // In a real system, this would update the configuration
            // For now, we'll log the advancement
            info!(
                "ROLLOUT ADVANCED: {} -> {}%",
                current_percentage, new_percentage
            );
        } else {
            info!("Rollout already at 100%");
        }
    }
}

