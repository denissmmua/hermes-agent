/// Slack platform adapter for the gateway
use async_trait::async_trait;
use hermes_core::AgentResult;
use crate::platforms::{Platform, PlatformCapabilities, PlatformMessage};

pub struct SlackPlatform {
    pub bot_token: String,
    pub signing_secret: String,
}

impl SlackPlatform {
    pub fn new(bot_token: &str, signing_secret: &str) -> Self {
        Self {
            bot_token: bot_token.to_string(),
            signing_secret: signing_secret.to_string(),
        }
    }
}

#[async_trait]
impl Platform for SlackPlatform {
    fn name(&self) -> &str { "slack" }

    fn capabilities(&self) -> PlatformCapabilities {
        PlatformCapabilities {
            can_edit_messages: true,
            can_delete_messages: false,
            can_send_typing: true,
            supports_threads: true,
            supports_reactions: true,
            supports_markdown: true,
            supports_code_blocks: true,
            supports_images: true,
        }
    }

    async fn send_message(&self, channel: &str, text: &str) -> AgentResult<String> {
        let client = reqwest::Client::new();
        let resp = client.post("https://slack.com/api/chat.postMessage")
            .header("Authorization", format!("Bearer {}", self.bot_token))
            .json(&serde_json::json!({
                "channel": channel,
                "text": text,
                "mrkdwn": true,
            }))
            .send()
            .await
            .map_err(|e| hermes_core::AgentError::Provider(format!("Slack: {}", e)))?;
        let body: serde_json::Value = resp.json().await.unwrap_or_default();
        Ok(body.to_string())
    }

    async fn edit_message(&self, channel: &str, ts: &str, text: &str) -> AgentResult<()> {
        let client = reqwest::Client::new();
        client.post("https://slack.com/api/chat.update")
            .header("Authorization", format!("Bearer {}", self.bot_token))
            .json(&serde_json::json!({
                "channel": channel,
                "ts": ts,
                "text": text,
            }))
            .send()
            .await
            .map_err(|e| hermes_core::AgentError::Provider(format!("Slack: {}", e)))?;
        Ok(())
    }

    async fn delete_message(&self, _channel: &str, _ts: &str) -> AgentResult<()> {
        // Slack doesn't allow bots to delete messages easily
        Ok(())
    }

    async fn send_typing(&self, channel: &str) -> AgentResult<()> {
        let client = reqwest::Client::new();
        client.post("https://slack.com/api/chat.postMessage")
            .header("Authorization", format!("Bearer {}", self.bot_token))
            .json(&serde_json::json!({
                "channel": channel,
                "text": "...",
            }))
            .send()
            .await
            .ok();
        Ok(())
    }

    async fn start(&self) -> AgentResult<()> {
        Ok(()) // Slack uses Events API
    }
}
