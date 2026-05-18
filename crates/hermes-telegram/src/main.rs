use hermes_telegram::{TelegramBot, TelegramConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let bot_token = std::env::var("TELEGRAM_BOT_TOKEN")
        .expect("TELEGRAM_BOT_TOKEN required");
    let api_key = std::env::var("OPENAI_API_KEY")
        .or_else(|_| std::env::var("DEEPSEEK_API_KEY"))
        .expect("OPENAI_API_KEY or DEEPSEEK_API_KEY required");
    let base_url = std::env::var("OPENAI_BASE_URL")
        .or_else(|_| std::env::var("DEEPSEEK_BASE_URL"))
        .unwrap_or_else(|_| "https://api.deepseek.com".to_string());

    // Parse allowed chat IDs from env
    let allowed_chats: Vec<i64> = std::env::var("ALLOWED_CHAT_IDS")
        .unwrap_or_default()
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();

    let config = TelegramConfig {
        bot_token,
        allowed_chat_ids: allowed_chats,
        model: std::env::var("HERMES_MODEL").unwrap_or_else(|_| "deepseek-chat".to_string()),
        base_url,
        api_key,
    };

    let bot = TelegramBot::new(config);
    println!("🟢 Starting Hermes Telegram bot...");
    bot.start().await
}
