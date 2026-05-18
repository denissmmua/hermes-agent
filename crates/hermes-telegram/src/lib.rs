use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info, warn};
use reqwest::Client;

use hermes_core::{AgentConfig, GatewayConfig, ToolRegistry};
use hermes_agent::AgentRuntime;
use hermes_gateway::OpenAIProvider;
use hermes_tools::{ShellTool, ReadFileTool, WriteFileTool, GrepTool, GitTool, WebFetchTool};

/// Telegram bot config
#[derive(Debug, Clone)]
pub struct TelegramConfig {
    pub bot_token: String,
    pub allowed_chat_ids: Vec<i64>,
    pub model: String,
    pub base_url: String,
    pub api_key: String,
}

/// Telegram message types
#[derive(serde::Deserialize, Debug)]
#[derive(Default)]
struct Update {
    #[serde(default)]
    update_id: i64,
    #[serde(default)]
    message: Option<Message>,
}

#[derive(serde::Deserialize, Debug)]
#[derive(Default)]
struct Message {
    #[serde(default)]
    message_id: i64,
    #[serde(default)]
    chat: Chat,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    from: Option<User>,
}

#[derive(serde::Deserialize, Debug)]
#[derive(Default)]
struct Chat {
    id: i64,
    #[serde(default)]
    r#type: Option<String>,
}

#[derive(serde::Deserialize, Debug)]
#[derive(Default)]
struct User {
    id: i64,
    #[serde(default)]
    username: Option<String>,
    #[serde(default)]
    first_name: Option<String>,
}

#[derive(serde::Serialize)]
struct SendMessage {
    chat_id: i64,
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    reply_to_message_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    parse_mode: Option<String>,
}

#[derive(serde::Deserialize)]
struct ApiResponse {
    ok: bool,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    result: Option<serde_json::Value>,
}

pub struct TelegramBot {
    client: Client,
    api_url: String,
    config: TelegramConfig,
    runtime: Arc<Mutex<Option<AgentRuntime>>>,
    last_update_id: Arc<Mutex<i64>>,
}

impl TelegramBot {
    pub fn new(config: TelegramConfig) -> Self {
        let api_url = format!("https://api.telegram.org/bot{}", config.bot_token);
        Self {
            client: Client::new(),
            api_url,
            config,
            runtime: Arc::new(Mutex::new(None)),
            last_update_id: Arc::new(Mutex::new(0)),
        }
    }

    pub async fn init_runtime(&self) -> anyhow::Result<()> {
        let gw_config = GatewayConfig {
            provider: "openai".to_string(),
            model: self.config.model.clone(),
            api_key: self.config.api_key.clone(),
            base_url: Some(self.config.base_url.clone()),
            ..Default::default()
        };

        let gateway = Arc::new(OpenAIProvider::new(gw_config));

        let mut tools = ToolRegistry::new();
        tools.register(Arc::new(ShellTool));
        tools.register(Arc::new(ReadFileTool));
        tools.register(Arc::new(WriteFileTool));
        tools.register(Arc::new(GrepTool));
        tools.register(Arc::new(GitTool));
        tools.register(Arc::new(WebFetchTool));

        let agent_cfg = AgentConfig {
            name: "hermes".to_string(),
            model: self.config.model.clone(),
            system_prompt: "\
You are Hermes, an AI coding agent running as a Telegram bot in Rust. \
You have tools for shell commands, file operations, git, web fetching, and text search. \
Use native function calling when you need to execute commands. \
Be concise, helpful, and respond in the user's language.".to_string(),
            ..Default::default()
        };

        let mut runtime = AgentRuntime::new(agent_cfg, gateway.clone());
        for name in tools.names() {
            if let Some(t) = tools.get(&name) {
                runtime.register_tool(t);
            }
        }

        *self.runtime.lock().await = Some(runtime);
        info!("Hermes Telegram bot runtime initialized");
        Ok(())
    }

    pub async fn start(&self) -> anyhow::Result<()> {
        info!("🟢 Hermes Telegram bot starting (long polling)...");
        self.init_runtime().await?;

        loop {
            if let Err(e) = self.poll_once().await {
                error!("Poll error: {}", e);
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }
    }

    async fn poll_once(&self) -> anyhow::Result<()> {
        let last_id = *self.last_update_id.lock().await;
        let offset = last_id + 1;

        let url = format!("{}/getUpdates", self.api_url);
        let resp = self.client
            .post(&url)
            .json(&serde_json::json!({
                "offset": offset,
                "timeout": 30,
                "allowed_updates": ["message"]
            }))
            .send()
            .await?;

        let api_resp: ApiResponse = resp.json().await?;
        if !api_resp.ok {
            warn!("Telegram API error: {:?}", api_resp.description);
            return Ok(());
        }

        let updates: Vec<Update> = serde_json::from_value(
            api_resp.result.unwrap_or_default()
        ).unwrap_or_default();

        for update in updates {
            self.handle_update(update).await?;
        }

        Ok(())
    }

    async fn handle_update(&self, update: Update) -> anyhow::Result<()> {
        // Track last update id
        {
            let mut last = self.last_update_id.lock().await;
            if update.update_id > *last {
                *last = update.update_id;
            }
        }

        let msg = match update.message {
            Some(m) => m,
            None => return Ok(()),
        };

        let chat_id = msg.chat.id;
        let text = match &msg.text {
            Some(t) if !t.is_empty() => t.clone(),
            _ => return Ok(()),
        };

        // Check if chat is allowed
        if !self.config.allowed_chat_ids.is_empty() && !self.config.allowed_chat_ids.contains(&chat_id) {
            return Ok(());
        }

        // Skip commands that start with /
        if text.starts_with('/') {
            match text.as_str() {
                "/start" | "/help" => {
                    self.send_text(chat_id, "🔷 Hermes Agent Rust — Telegram bot\n\nSend me any message and I'll respond as the AI agent.\n\nAvailable tools: shell, file, git, grep, web_fetch", msg.message_id).await?;
                }
                "/status" => {
                    self.send_text(chat_id, "✅ Hermes Telegram bot is running", msg.message_id).await?;
                }
                _ => {}
            }
            return Ok(());
        }

        info!("⚡ Message from {}: {}", chat_id, &text[..text.len().min(50)]);

        // Think indicator
        self.send_chat_action(chat_id, "typing").await.ok();

        // Process through agent
        let result = {
            let mut rt_guard = self.runtime.lock().await;
            match &mut *rt_guard {
                Some(runtime) => runtime.run_turn(&text).await,
                None => Err(hermes_core::AgentError::Unknown("Runtime not initialized".into())),
            }
        };

        match result {
            Ok(response) => {
                if !response.is_empty() {
                    self.send_text(chat_id, &response, msg.message_id).await?;
                }
                // Show tool results
                let rt_guard = self.runtime.lock().await;
                if let Some(runtime) = &*rt_guard {
                    let tool_msgs: Vec<_> = runtime.state().conversation.messages.iter()
                        .filter(|m| m.content.starts_with("Tool ") || m.content.starts_with("Result:"))
                        .collect();
                    for tm in tool_msgs {
                        let short = if tm.content.len() > 1000 {
                            format!("{}...\n[truncated {} chars]", &tm.content[..1000], tm.content.len() - 1000)
                        } else {
                            tm.content.clone()
                        };
                        self.send_text(chat_id, &short, msg.message_id).await.ok();
                    }
                }
            }
            Err(e) => {
                self.send_text(chat_id, &format!("⚠ Error: {}", e), msg.message_id).await?;
            }
        }

        Ok(())
    }

    async fn send_text(&self, chat_id: i64, text: &str, reply_id: i64) -> anyhow::Result<()> {
        let url = format!("{}/sendMessage", self.api_url);
        let payload = SendMessage {
            chat_id,
            text: text.to_string(),
            reply_to_message_id: Some(reply_id),
            parse_mode: None,
        };
        self.client.post(&url).json(&payload).send().await?;
        Ok(())
    }

    async fn send_chat_action(&self, chat_id: i64, action: &str) -> anyhow::Result<()> {
        let url = format!("{}/sendChatAction", self.api_url);
        self.client
            .post(&url)
            .json(&serde_json::json!({"chat_id": chat_id, "action": action}))
            .send()
            .await?;
        Ok(())
    }
}
