/// Discord platform adapter for the gateway
use async_trait::async_trait;
use hermes_core::AgentResult;
use crate::platforms::{Platform, PlatformCapabilities, PlatformMessage};

pub struct DiscordPlatform {
    pub bot_token: String,
    pub api_url: String,
}

impl DiscordPlatform {
    pub fn new(bot_token: &str) -> Self {
        Self {
            bot_token: bot_token.to_string(),
            api_url: "https://discord.com/api/v10".to_string(),
        }
    }
}

#[async_trait]
impl Platform for DiscordPlatform {
    fn name(&self) -> &str { "discord" }

    fn capabilities(&self) -> PlatformCapabilities {
        PlatformCapabilities {
            can_edit_messages: true,
            can_delete_messages: true,
            can_send_typing: true,
            supports_threads: true,
            supports_reactions: true,
            supports_markdown: false,
            supports_code_blocks: true,
            supports_images: true,
        }
    }

    async fn send_message(&self, channel_id: &str, text: &str) -> AgentResult<String> {
        let client = reqwest::Client::new();
        let url = format!("{}/channels/{}/messages", self.api_url, channel_id);
        let resp = client.post(&url)
            .header("Authorization", format!("Bot {}", self.bot_token))
            .json(&serde_json::json!({"content": text}))
            .send()
            .await
            .map_err(|e| hermes_core::AgentError::Provider(format!("Discord: {}", e)))?;
        let body = resp.text().await.unwrap_or_default();
        Ok(body)
    }

    async fn edit_message(&self, channel_id: &str, message_id: &str, text: &str) -> AgentResult<()> {
        let client = reqwest::Client::new();
        let url = format!("{}/channels/{}/messages/{}", self.api_url, channel_id, message_id);
        client.patch(&url)
            .header("Authorization", format!("Bot {}", self.bot_token))
            .json(&serde_json::json!({"content": text}))
            .send()
            .await
            .map_err(|e| hermes_core::AgentError::Provider(format!("Discord: {}", e)))?;
        Ok(())
    }

    async fn delete_message(&self, channel_id: &str, message_id: &str) -> AgentResult<()> {
        let client = reqwest::Client::new();
        let url = format!("{}/channels/{}/messages/{}", self.api_url, channel_id, message_id);
        client.delete(&url)
            .header("Authorization", format!("Bot {}", self.bot_token))
            .send()
            .await
            .map_err(|e| hermes_core::AgentError::Provider(format!("Discord: {}", e)))?;
        Ok(())
    }

    async fn send_typing(&self, channel_id: &str) -> AgentResult<()> {
        let client = reqwest::Client::new();
        let url = format!("{}/channels/{}/typing", self.api_url, channel_id);
        client.post(&url)
            .header("Authorization", format!("Bot {}", self.bot_token))
            .send()
            .await
            .ok();
        Ok(())
    }

    async fn start(&self) -> AgentResult<()> {
        Ok(()) // Discord uses Gateway API, not webhook
    }
}
