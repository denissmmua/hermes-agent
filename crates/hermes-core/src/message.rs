use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: Uuid,
    pub role: Role,
    pub content: String,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub tool_result: Option<ToolResultMeta>,
    pub timestamp: DateTime<Utc>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResultMeta {
    pub call_id: String,
    pub tool_name: String,
    pub output: String,
    pub success: bool,
    pub duration_ms: u64,
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            role: Role::System,
            content: content.into(),
            tool_calls: None,
            tool_result: None,
            timestamp: Utc::now(),
            metadata: None,
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            role: Role::User,
            content: content.into(),
            tool_calls: None,
            tool_result: None,
            timestamp: Utc::now(),
            metadata: None,
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            role: Role::Assistant,
            content: content.into(),
            tool_calls: None,
            tool_result: None,
            timestamp: Utc::now(),
            metadata: None,
        }
    }

    pub fn with_tool_calls(mut self, calls: Vec<ToolCall>) -> Self {
        self.tool_calls = Some(calls);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: Uuid,
    pub messages: Vec<Message>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub metadata: Option<serde_json::Value>,
}

impl Conversation {
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            messages: Vec::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            metadata: None,
        }
    }

    pub fn push(&mut self, msg: Message) {
        self.updated_at = Utc::now();
        self.messages.push(msg);
    }

    pub fn context_length(&self) -> usize {
        self.messages.iter().map(|m| m.content.len()).sum()
    }

    pub fn truncate(&mut self, max_tokens: usize) {
        while self.context_length() > max_tokens && self.messages.len() > 1 {
            // Keep system messages, drop oldest non-system
            if let Some(pos) = self.messages.iter().position(|m| matches!(m.role, Role::User)) {
                self.messages.remove(pos);
            } else {
                break;
            }
        }
    }
}

impl Default for Conversation {
    fn default() -> Self {
        Self::new()
    }
}
