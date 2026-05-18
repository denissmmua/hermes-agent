use std::time::Duration;
use tokio::time::sleep;
use tracing::warn;

use hermes_core::{AgentError, AgentResult};

pub struct RetryConfig {
    pub max_retries: u32,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay_ms: 1000,
            max_delay_ms: 30000,
        }
    }
}

pub async fn with_retry<F, Fut, T>(config: &RetryConfig, operation: F) -> AgentResult<T>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = AgentResult<T>>,
{
    let mut last_err = None;
    for attempt in 0..=config.max_retries {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                let is_retryable = matches!(
                    &e,
                    AgentError::RateLimited(_) | AgentError::Provider(_)
                );
                if !is_retryable || attempt == config.max_retries {
                    return Err(e);
                }
                let delay = config.base_delay_ms * 2u64.pow(attempt);
                let delay = delay.min(config.max_delay_ms);
                warn!(
                    "Retry {}/{} after {}ms: {}",
                    attempt + 1,
                    config.max_retries,
                    delay,
                    e
                );
                sleep(Duration::from_millis(delay)).await;
                last_err = Some(e);
            }
        }
    }
    Err(last_err.unwrap_or_else(|| AgentError::Unknown("Retry failed".into())))
}
