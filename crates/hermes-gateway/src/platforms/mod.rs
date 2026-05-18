use async_trait::async_trait;
use hermes_core::AgentResult;

/// A message from a platform
#[derive(Debug, Clone)]
pub struct PlatformMessage {
    pub id: String,
    pub text: String,
    pub user_id: String,
    pub user_name: String,
    pub chat_id: String,
    pub chat_name: String,
    pub platform: String,
    pub reply_to: Option<String>,
}

/// Platform capability flags
#[derive(Debug, Clone)]
pub struct PlatformCapabilities {
    pub can_edit_messages: bool,
    pub can_delete_messages: bool,
    pub can_send_typing: bool,
    pub supports_threads: bool,
    pub supports_reactions: bool,
    pub supports_markdown: bool,
    pub supports_code_blocks: bool,
    pub supports_images: bool,
}

/// Abstract platform interface for messaging platforms
#[async_trait]
pub trait Platform: Send + Sync {
    fn name(&self) -> &str;
    fn capabilities(&self) -> PlatformCapabilities;

    async fn send_message(&self, chat_id: &str, text: &str) -> AgentResult<String>;
    async fn edit_message(&self, chat_id: &str, message_id: &str, text: &str) -> AgentResult<()>;
    async fn delete_message(&self, chat_id: &str, message_id: &str) -> AgentResult<()>;
    async fn send_typing(&self, chat_id: &str) -> AgentResult<()>;
    async fn start(&self) -> AgentResult<()>;
}
