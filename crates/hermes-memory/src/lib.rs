use chrono::{DateTime, Utc};
use hermes_core::AgentResult;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: String,
    pub content: String,
    pub kind: String, // fact, preference, lesson, context
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub importance: u8, // 1-10
    pub tags: Vec<String>,
}

pub struct MemoryStore {
    entries: Vec<MemoryEntry>,
    file_path: PathBuf,
    max_chars: usize,
}

impl MemoryStore {
    pub fn new(file_path: impl Into<PathBuf>, max_chars: usize) -> Self {
        Self {
            entries: Vec::new(),
            file_path: file_path.into(),
            max_chars,
        }
    }

    pub async fn load(&mut self) -> AgentResult<()> {
        if self.file_path.exists() {
            let data = tokio::fs::read_to_string(&self.file_path).await?;
            self.entries = serde_json::from_str(&data).unwrap_or_default();
        }
        Ok(())
    }

    pub async fn save(&self) -> AgentResult<()> {
        if let Some(parent) = self.file_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let data = serde_json::to_string_pretty(&self.entries)?;
        // Truncate to max_chars
        let truncated = if data.len() > self.max_chars {
            let mut trimmed: Vec<MemoryEntry> = self.entries.clone();
            trimmed.sort_by(|a, b| b.importance.cmp(&a.importance));
            // Keep top entries that fit
            let mut result = Vec::new();
            let mut size = 2; // for []
            for entry in &trimmed {
                let entry_size = serde_json::to_string(entry).map(|s| s.len()).unwrap_or(0);
                if size + entry_size + 1 > self.max_chars {
                    break;
                }
                size += entry_size + 1;
                result.push(entry.clone());
            }
            serde_json::to_string_pretty(&result)?
        } else {
            data
        };
        tokio::fs::write(&self.file_path, &truncated).await?;
        Ok(())
    }

    pub fn add(&mut self, content: String, kind: String, importance: u8, tags: Vec<String>) {
        let now = Utc::now();
        let entry = MemoryEntry {
            id: uuid::Uuid::new_v4().to_string(),
            content,
            kind,
            created_at: now,
            updated_at: now,
            importance,
            tags,
        };
        self.entries.push(entry);
    }

    pub fn search(&self, query: &str) -> Vec<&MemoryEntry> {
        let query_lower = query.to_lowercase();
        self.entries
            .iter()
            .filter(|e| {
                e.content.to_lowercase().contains(&query_lower)
                    || e.tags.iter().any(|t| t.to_lowercase().contains(&query_lower))
            })
            .collect()
    }

    pub fn all(&self) -> &[MemoryEntry] {
        &self.entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
