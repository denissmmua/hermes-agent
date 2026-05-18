pub mod telegram;
pub mod discord;
pub mod slack;

pub use telegram::*;
pub use discord::*;
pub use slack::*;

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

/// Default no-op platform (for tests)
pub struct NoopPlatform;

#[async_trait]
impl Platform for NoopPlatform {
    fn name(&self) -> &str { "noop" }
    fn capabilities(&self) -> PlatformCapabilities {
        PlatformCapabilities {
            can_edit_messages: false, can_delete_messages: false,
            can_send_typing: false, supports_threads: false,
            supports_reactions: false, supports_markdown: false,
            supports_code_blocks: false, supports_images: false,
        }
    }
    async fn send_message(&self, _: &str, _: &str) -> AgentResult<String> { Ok("ok".to_string()) }
    async fn edit_message(&self, _: &str, _: &str, _: &str) -> AgentResult<()> { Ok(()) }
    async fn delete_message(&self, _: &str, _: &str) -> AgentResult<()> { Ok(()) }
    async fn send_typing(&self, _: &str) -> AgentResult<()> { Ok(()) }
    async fn start(&self) -> AgentResult<()> { Ok(()) }
}
