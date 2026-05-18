use hermes_web::{start_webui, WebUIConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let cfg = WebUIConfig {
        port: 9090,
        host: "0.0.0.0".to_string(),
        model: std::env::var("HERMES_MODEL").unwrap_or_else(|_| "deepseek-chat".to_string()),
        api_key: std::env::var("OPENAI_API_KEY")
            .or_else(|_| std::env::var("DEEPSEEK_API_KEY"))
            .unwrap_or_default(),
        base_url: std::env::var("OPENAI_BASE_URL")
            .or_else(|_| std::env::var("DEEPSEEK_BASE_URL"))
            .unwrap_or_else(|_| "https://api.deepseek.com".to_string()),
    };
    println!("Hermes Web UI: http://{}:{}", cfg.host, cfg.port);
    start_webui(cfg).await
}
