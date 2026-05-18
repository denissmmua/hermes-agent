use hermes_core::{HermesConfig};
use hermes_telegram::{TelegramBot, TelegramConfig};

/// Load gateway config from config file, falling back to env vars
fn load_gateway_config() -> (String, String, String) {
    let config_path = shellexpand::tilde("~/.hermes/config.yaml");
    if let Ok(c) = std::fs::read_to_string(config_path.as_ref()) {
        if let Ok(cfg) = serde_yaml::from_str::<HermesConfig>(&c) {
            // Try custom providers first (has full DeepSeek config)
            for cp in &cfg.custom_providers {
                if !cp.api_key.is_empty() {
                    return (cp.api_key.clone(), cp.base_url.clone(), cp.model.clone());
                }
            }
            // Fall back to model section
            if let Some(ak) = &cfg.model.api_key {
                if !ak.is_empty() {
                    let base = match cfg.model.base_url.as_ref() {
                        Some(u) if !u.is_empty() => u.clone(),
                        _ => "https://api.deepseek.com".to_string(),
                    };
                    let model = if cfg.model.default == "gpt-5.5" {
                        "deepseek-chat".to_string()
                    } else {
                        cfg.model.default.clone()
                    };
                    return (ak.clone(), base, model);
                }
            }
        }
    }
    // Fall back to env vars
    let api_key = std::env::var("OPENAI_API_KEY")
        .or_else(|_| std::env::var("DEEPSEEK_API_KEY"))
        .unwrap_or_default();
    let base_url = std::env::var("OPENAI_BASE_URL")
        .or_else(|_| std::env::var("DEEPSEEK_BASE_URL"))
        .unwrap_or_else(|_| "https://api.deepseek.com".to_string());
    let model = std::env::var("HERMES_MODEL").unwrap_or_else(|_| "deepseek-chat".to_string());
    (api_key, base_url, model)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let bot_token = std::env::var("TELEGRAM_BOT_TOKEN")
        .or_else(|_| {
            std::fs::read_to_string("/root/.hermes/telegram-token.txt")
                .map(|s| s.trim().to_string())
        })
        .expect("TELEGRAM_BOT_TOKEN required (env or /root/.hermes/telegram-token.txt)");

    let (api_key, base_url, model) = load_gateway_config();

    // Parse allowed chat IDs from env (optional)
    let allowed_chats: Vec<i64> = std::env::var("ALLOWED_CHAT_IDS")
        .unwrap_or_default()
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();

    let config = TelegramConfig {
        bot_token,
        allowed_chat_ids: allowed_chats,
        model,
        base_url,
        api_key,
    };

    let bot = TelegramBot::new(config);
    println!("\u{1f534} Starting Hermes Telegram bot...");
    bot.start().await
}
