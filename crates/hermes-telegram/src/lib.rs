use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info, warn};
use reqwest::Client;

use hermes_core::{AgentConfig, GatewayConfig, ToolRegistry};
use hermes_gateway::OpenAIProvider;
use hermes_agent::AgentRuntime;
use hermes_tools::{ShellTool, ReadFileTool, WriteFileTool, GrepTool, GitTool, WebFetchTool};

#[derive(Debug, Clone)]
pub struct TelegramConfig {
    pub bot_token: String,
    pub allowed_chat_ids: Vec<i64>,
    pub model: String,
    pub base_url: String,
    pub api_key: String,
}

#[derive(serde::Deserialize, Debug)]
struct Update {
    update_id: i64,
    #[serde(default)]
    message: Option<Message>,
}

#[derive(serde::Deserialize, Debug, Default)]
struct Message {
    message_id: i64,
    chat: Chat,
    text: Option<String>,
    from: Option<User>,
    #[serde(default)]
    message_thread_id: Option<i64>,
}

#[derive(serde::Deserialize, Debug, Default)]
struct Chat { id: i64, r#type: Option<String> }

#[derive(serde::Deserialize, Debug, Default)]
struct User { id: i64, username: Option<String>, first_name: Option<String> }

#[derive(serde::Serialize)]
struct SendMsg {
    chat_id: i64,
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    reply_to_message_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message_thread_id: Option<i64>,
}

#[derive(serde::Serialize)]
struct EditMsg {
    chat_id: i64,
    message_id: i64,
    text: String,
}

#[derive(serde::Deserialize)]
struct ApiResponse { ok: bool, result: Option<serde_json::Value> }

struct BotState {
    runtime: AgentRuntime,
    bot: Arc<TelegramBotInner>,
}

pub struct TelegramBot {
    inner: Arc<TelegramBotInner>,
    state: Arc<Mutex<Option<BotState>>>,
    last_update_id: Arc<Mutex<i64>>,
}

struct TelegramBotInner {
    client: Client,
    api_url: String,
    config: TelegramConfig,
}

impl TelegramBot {
    pub fn new(config: TelegramConfig) -> Self {
        let api_url = format!("https://api.telegram.org/bot{}", config.bot_token);
        Self {
            inner: Arc::new(TelegramBotInner {
                client: Client::new(),
                api_url,
                config,
            }),
            state: Arc::new(Mutex::new(None)),
            last_update_id: Arc::new(Mutex::new(0)),
        }
    }

    pub async fn start(&self) -> anyhow::Result<()> {
        info!("Starting Hermes Telegram bot...");
        self.init_state().await?;
        loop {
            if let Err(e) = self.poll_once().await {
                error!("Poll: {}", e);
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        }
    }

    async fn init_state(&self) -> anyhow::Result<()> {
        let gw_config = GatewayConfig {
            provider: "openai".to_string(),
            model: self.inner.config.model.clone(),
            api_key: self.inner.config.api_key.clone(),
            base_url: Some(self.inner.config.base_url.clone()),
            ..Default::default()
        };

        let gateway = Arc::new(OpenAIProvider::new(gw_config));

        let mut tools = ToolRegistry::new();
        for t in [Arc::new(ShellTool) as Arc<dyn hermes_core::Tool>,
                  Arc::new(ReadFileTool), Arc::new(WriteFileTool),
                  Arc::new(GrepTool), Arc::new(GitTool), Arc::new(WebFetchTool)] {
            tools.register(t);
        }

        let mut runtime = AgentRuntime::new(AgentConfig {
            name: "hermes".to_string(),
            model: self.inner.config.model.clone(),
            system_prompt: "\
You are Hermes, an AI agent in Rust running as a Telegram bot. \
Tools: shell, read_file, write_file, grep, git, web_fetch. \
Use function calling when needed. Be concise.".to_string(),
            ..Default::default()
        }, gateway.clone());

        for name in tools.names() {
            if let Some(t) = tools.get(&name) { runtime.register_tool(t); }
        }

        *self.state.lock().await = Some(BotState {
            runtime,
            bot: self.inner.clone(),
        });

        info!("Runtime ready");
        Ok(())
    }

    async fn poll_once(&self) -> anyhow::Result<()> {
        let offset = *self.last_update_id.lock().await + 1;
        let resp = self.inner.client
            .post(format!("{}/getUpdates", self.inner.api_url))
            .json(&serde_json::json!({"offset": offset, "timeout": 30, "allowed_updates": ["message"]}))
            .send().await?;

        let updates: Vec<Update> = serde_json::from_value(
            match resp.json::<ApiResponse>().await {
                Ok(a) => a.result.unwrap_or_default(),
                Err(_) => return Ok(()),
            }
        ).unwrap_or_default();

        for u in updates {
            let uid = u.update_id;
            *self.last_update_id.lock().await = uid;
            if let Err(e) = self.handle(u).await {
                error!("Update {}: {}", uid, e);
            }
        }
        Ok(())
    }

    async fn handle(&self, update: Update) -> anyhow::Result<()> {
        let msg = match update.message { Some(m) => m, None => return Ok(()) };
        let text = match &msg.text { Some(t) if !t.is_empty() => t.clone(), _ => return Ok(()) };
        let chat_id = msg.chat.id;
        let thread_id = msg.message_thread_id;
        let reply_id = msg.message_id;

        // Skip own messages
        if let Some(ref from) = msg.from {
            if from.id == 8894352489 { return Ok(()); }
        }

        // Filter
        if !self.inner.config.allowed_chat_ids.is_empty() &&
           !self.inner.config.allowed_chat_ids.contains(&chat_id) {
            return Ok(());
        }

        // Commands
        if text.starts_with('/') {
            return self.handle_command(chat_id, thread_id, &text, reply_id).await;
        }

        info!("⚡ {}: {}", chat_id, &text[..text.len().min(60)]);
        self.send_action(chat_id, "typing").await.ok();

        // Send "thinking" message first, then edit it
        let status_msg = self.send_msg(chat_id, thread_id, Some(reply_id), "🤔 Думаю...").await?;

        let result = {
            let mut guard = self.state.lock().await;
            match &mut *guard {
                Some(s) => s.runtime.run_turn(&text).await,
                None => return Ok(()),
            }
        };

        match result {
            Ok(response) => {
                if !response.is_empty() {
                    // Edit the status message with the real response
                    self.edit_msg(chat_id, status_msg, &response).await?;
                }
                // Show tool results
                let guard = self.state.lock().await;
                if let Some(s) = &*guard {
                    for m in s.runtime.state().conversation.messages.iter().rev().take(3) {
                        if (m.content.starts_with("Result:") || m.content.starts_with("Tool ")) &&
                            !response.contains(&m.content) {
                            let short = if m.content.len() > 400 {
                                format!("{}...", &m.content[..400])
                            } else { m.content.clone() };
                            self.send_msg(chat_id, thread_id, Some(reply_id), &short).await.ok();
                        }
                    }
                }
            }
            Err(e) => {
                self.edit_msg(chat_id, status_msg, &format!("⚠ Error: {}", e)).await?;
            }
        }

        Ok(())
    }

    async fn handle_command(&self, chat_id: i64, thread_id: Option<i64>, cmd: &str, reply_id: i64) -> anyhow::Result<()> {
        let response = match cmd {
            "/start" => "🔷 **Hermes Agent Rust** — Telegram bot\n\nНапиши что-нибудь — я отвечу.".to_string(),
            "/status" => "✅ Бот работает\nМодель: deepseek-chat\nИнструменты: shell, файлы, git, grep, web".to_string(),
            _ => format!("Неизвестная команда: {}", cmd),
        };
        self.send_msg(chat_id, thread_id, Some(reply_id), &response).await?;
        Ok(())
    }

    async fn send_msg(&self, chat_id: i64, thread_id: Option<i64>, reply_to: Option<i64>, text: &str) -> anyhow::Result<i64> {
        let payload = SendMsg {
            chat_id, text: text.to_string(),
            reply_to_message_id: reply_to,
            message_thread_id: thread_id,
        };
        let resp = self.inner.client
            .post(format!("{}/sendMessage", self.inner.api_url))
            .json(&payload).send().await?;
        let data: serde_json::Value = resp.json().await?;
        Ok(data["result"]["message_id"].as_i64().unwrap_or(0))
    }

    async fn edit_msg(&self, chat_id: i64, msg_id: i64, text: &str) -> anyhow::Result<()> {
        self.inner.client
            .post(format!("{}/editMessageText", self.inner.api_url))
            .json(&EditMsg { chat_id, message_id: msg_id, text: text.to_string() })
            .send().await?;
        Ok(())
    }

    async fn send_action(&self, chat_id: i64, action: &str) -> anyhow::Result<()> {
        self.inner.client
            .post(format!("{}/sendChatAction", self.inner.api_url))
            .json(&serde_json::json!({"chat_id": chat_id, "action": action}))
            .send().await?;
        Ok(())
    }
}
