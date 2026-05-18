/// Telegram platform adapter for the gateway
use async_trait::async_trait;
use hermes_core::AgentResult;
use crate::platforms::{Platform, PlatformCapabilities, PlatformMessage};
use crate::delivery::DeliveryManager;

pub struct TelegramPlatform {
    pub bot_token: String,
    pub api_url: String,
    pub delivery: DeliveryManager,
    pub allowed_chat_ids: Vec<i64>,
}

impl TelegramPlatform {
    pub fn new(bot_token: &str) -> Self {
        let api_url = format!("https://api.telegram.org/bot{}", bot_token);
        Self {
            bot_token: bot_token.to_string(),
            api_url,
            delivery: DeliveryManager::new(),
            allowed_chat_ids: Vec::new(),
        }
    }

    async fn api_call(&self, method: &str, payload: serde_json::Value) -> AgentResult<String> {
        let client = reqwest::Client::new();
        let url = format!("{}/{}", self.api_url, method);
        let resp = client.post(&url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| hermes_core::AgentError::Provider(format!("Telegram API: {}", e)))?;
        let text = resp.text().await
            .map_err(|e| hermes_core::AgentError::Provider(format!("Read: {}", e)))?;
        Ok(text)
    }
}

#[async_trait]
impl Platform for TelegramPlatform {
    fn name(&self) -> &str { "telegram" }

    fn capabilities(&self) -> PlatformCapabilities {
        PlatformCapabilities {
            can_edit_messages: true,
            can_delete_messages: true,
            can_send_typing: true,
            supports_threads: true,
            supports_reactions: true,
            supports_markdown: true,
            supports_code_blocks: true,
            supports_images: true,
        }
    }

    async fn send_message(&self, chat_id: &str, text: &str) -> AgentResult<String> {
        let payload = serde_json::json!({
            "chat_id": chat_id,
            "text": text,
            "parse_mode": "Markdown",
        });
        let result = self.api_call("sendMessage", payload).await?;
        Ok(result)
    }

    async fn edit_message(&self, chat_id: &str, message_id: &str, text: &str) -> AgentResult<()> {
        let payload = serde_json::json!({
            "chat_id": chat_id,
            "message_id": message_id,
            "text": text,
            "parse_mode": "Markdown",
        });
        self.api_call("editMessageText", payload).await?;
        Ok(())
    }

    async fn delete_message(&self, chat_id: &str, message_id: &str) -> AgentResult<()> {
        let payload = serde_json::json!({
            "chat_id": chat_id,
            "message_id": message_id,
        });
        self.api_call("deleteMessage", payload).await?;
        Ok(())
    }

    async fn send_typing(&self, chat_id: &str) -> AgentResult<()> {
        let payload = serde_json::json!({
            "chat_id": chat_id,
            "action": "typing",
        });
        self.api_call("sendChatAction", payload).await?;
        Ok(())
    }

    async fn start(&self) -> AgentResult<()> {
        // Set webhook
        let payload = serde_json::json!({
            "url": "",  // Will be set by gateway
            "allowed_updates": ["message", "callback_query"],
        });
        self.api_call("setWebhook", payload).await?;
        Ok(())
    }
}
