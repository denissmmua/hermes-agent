use chrono::{DateTime, Utc};
use hermes_core::AgentResult;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{interval, Duration};
use tracing::{error, info};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronJob {
    pub id: String,
    pub name: String,
    pub schedule: String, // cron expression
    pub command: String,
    pub enabled: bool,
    pub last_run: Option<DateTime<Utc>>,
    pub last_output: Option<String>,
}

pub trait JobHandler: Send + Sync {
    fn name(&self) -> &str;
    fn execute(&self) -> impl std::future::Future<Output = AgentResult<String>> + Send;
}

pub struct CronEngine {
    jobs: Arc<Mutex<HashMap<String, CronJob>>>,
    running: Arc<std::sync::atomic::AtomicBool>,
}

impl CronEngine {
    pub fn new() -> Self {
        Self {
            jobs: Arc::new(Mutex::new(HashMap::new())),
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    pub async fn register(&self, job: CronJob) {
        let mut jobs = self.jobs.lock().await;
        jobs.insert(job.id.clone(), job);
    }

    pub async fn start(&self) {
        self.running
            .store(true, std::sync::atomic::Ordering::Relaxed);
        let jobs = self.jobs.clone();
        let running = self.running.clone();

        tokio::spawn(async move {
            let mut tick = interval(Duration::from_secs(60));
            while running.load(std::sync::atomic::Ordering::Relaxed) {
                tick.tick().await;
                let now = Utc::now();
                let mut jobs_lock = jobs.lock().await;
                for (_, job) in jobs_lock.iter_mut() {
                    if !job.enabled {
                        continue;
                    }
                    // Simple minute-based check
                    let should_run = job.last_run.map_or(true, |last| {
                        (now - last).num_minutes() >= parse_minutes(&job.schedule).unwrap_or(60)
                    });
                    if should_run {
                        info!("Running cron job: {}", job.name);
                        job.last_run = Some(now);
                        // Execute via shell
                        let output = tokio::process::Command::new("sh")
                            .arg("-c")
                            .arg(&job.command)
                            .output()
                            .await;
                        match output {
                            Ok(out) => {
                                let result = String::from_utf8_lossy(&out.stdout).to_string();
                                job.last_output = Some(result.clone());
                                info!("Cron '{}' completed", job.name);
                            }
                            Err(e) => {
                                error!("Cron '{}' failed: {}", job.name, e);
                                job.last_output = Some(format!("Error: {}", e));
                            }
                        }
                    }
                }
            }
        });
    }

    pub async fn stop(&self) {
        self.running
            .store(false, std::sync::atomic::Ordering::Relaxed);
    }

    pub async fn list_jobs(&self) -> Vec<CronJob> {
        self.jobs.lock().await.values().cloned().collect()
    }
}

fn parse_minutes(expr: &str) -> Option<i64> {
    let expr = expr.trim();
    if expr == "* * * * *" {
        return Some(1);
    }
    if let Some(min) = expr.split_whitespace().next() {
        if min == "*" {
            return Some(1);
        }
        return min.parse::<i64>().ok();
    }
    None
}
