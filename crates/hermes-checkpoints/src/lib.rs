use chrono::{DateTime, Utc};
use hermes_core::{AgentResult, Conversation};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub id: String,
    pub session_id: String,
    pub turn: u32,
    pub created_at: DateTime<Utc>,
    pub summary: String,
    pub conversation: Conversation,
    pub metadata: Option<serde_json::Value>,
}

pub struct CheckpointManager {
    dir: PathBuf,
    max_snapshots: usize,
    max_total_size_mb: usize,
}

impl CheckpointManager {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: dir.into(),
            max_snapshots: 50,
            max_total_size_mb: 500,
        }
    }

    pub fn checkpoint_path(&self, id: &str) -> PathBuf {
        self.dir.join(format!("ckpt_{}.json", id))
    }

    pub async fn save(&self, checkpoint: &Checkpoint) -> AgentResult<()> {
        tokio::fs::create_dir_all(&self.dir).await?;
        let path = self.checkpoint_path(&checkpoint.id);
        let data = serde_json::to_string_pretty(checkpoint)?;
        tokio::fs::write(&path, &data).await?;
        Ok(())
    }

    pub async fn load(&self, id: &str) -> AgentResult<Checkpoint> {
        let path = self.checkpoint_path(id);
        let data = tokio::fs::read_to_string(&path).await?;
        let checkpoint: Checkpoint = serde_json::from_str(&data)?;
        Ok(checkpoint)
    }

    pub async fn list(&self) -> AgentResult<Vec<Checkpoint>> {
        let mut checkpoints = Vec::new();
        let mut dir = tokio::fs::read_dir(&self.dir).await?;
        while let Some(entry) = dir.next_entry().await? {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "json") {
                if let Ok(data) = tokio::fs::read_to_string(&path).await {
                    if let Ok(ckpt) = serde_json::from_str::<Checkpoint>(&data) {
                        checkpoints.push(ckpt);
                    }
                }
            }
        }
        checkpoints.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(checkpoints)
    }

    pub async fn prune(&self) -> AgentResult<usize> {
        let mut checkpoints = self.list().await?;
        if checkpoints.len() <= self.max_snapshots {
            return Ok(0);
        }
        checkpoints.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        let to_remove = checkpoints.len() - self.max_snapshots;
        for ckpt in checkpoints.iter().take(to_remove) {
            let path = self.checkpoint_path(&ckpt.id);
            let _ = tokio::fs::remove_file(&path).await;
        }
        Ok(to_remove)
    }
}
