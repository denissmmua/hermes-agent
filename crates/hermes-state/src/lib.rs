use hermes_core::{AgentResult, Conversation};
use std::path::{Path, PathBuf};

pub struct StateManager {
    state_dir: PathBuf,
}

impl StateManager {
    pub fn new(state_dir: impl Into<PathBuf>) -> Self {
        Self {
            state_dir: state_dir.into(),
        }
    }

    pub fn session_path(&self, session_id: &str) -> PathBuf {
        self.state_dir.join(format!("session_{}.json", session_id))
    }

    pub async fn save_session(&self, session_id: &str, conv: &Conversation) -> AgentResult<()> {
        tokio::fs::create_dir_all(&self.state_dir).await?;
        let path = self.session_path(session_id);
        let data = serde_json::to_string_pretty(conv)?;
        tokio::fs::write(&path, &data).await?;
        Ok(())
    }

    pub async fn load_session(&self, session_id: &str) -> AgentResult<Conversation> {
        let path = self.session_path(session_id);
        let data = tokio::fs::read_to_string(&path).await?;
        let conv: Conversation = serde_json::from_str(&data)?;
        Ok(conv)
    }

    pub async fn list_sessions(&self) -> AgentResult<Vec<String>> {
        let mut sessions = Vec::new();
        let mut dir = tokio::fs::read_dir(&self.state_dir).await?;
        while let Some(entry) = dir.next_entry().await? {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("session_") && name.ends_with(".json") {
                let id = name
                    .strip_prefix("session_")
                    .and_then(|s| s.strip_suffix(".json"))
                    .map(|s| s.to_string());
                if let Some(id) = id {
                    sessions.push(id);
                }
            }
        }
        Ok(sessions)
    }
}
