use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// State manager for sessions, memory, config persistence
pub struct StateManager {
    base_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: String,
    pub created_at: String,
    pub message_count: usize,
    pub model: String,
    pub summary: Option<String>,
}

impl StateManager {
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self { base_dir: base_dir.into() }
    }

    pub fn sessions_dir(&self) -> PathBuf { self.base_dir.join("sessions") }
    pub fn memory_dir(&self) -> PathBuf { self.base_dir.join("memory") }
    pub fn config_dir(&self) -> PathBuf { self.base_dir.join("config") }

    pub fn ensure_dirs(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(self.sessions_dir())?;
        std::fs::create_dir_all(self.memory_dir())?;
        std::fs::create_dir_all(self.config_dir())?;
        Ok(())
    }

    pub fn list_sessions(&self) -> Vec<SessionInfo> {
        let mut sessions = Vec::new();
        let dir = match std::fs::read_dir(self.sessions_dir()) {
            Ok(d) => d, Err(_) => return sessions,
        };
        for entry in dir.flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "json") {
                if let Ok(data) = std::fs::read_to_string(&path) {
                    if let Ok(info) = serde_json::from_str::<SessionInfo>(&data) {
                        sessions.push(info);
                    }
                }
            }
        }
        sessions
    }

    pub fn save_session(&self, info: &SessionInfo) -> std::io::Result<()> {
        self.ensure_dirs()?;
        let path = self.sessions_dir().join(format!("{}.json", info.id));
        let data = serde_json::to_string_pretty(info)?;
        std::fs::write(&path, &data)?;
        Ok(())
    }
}
