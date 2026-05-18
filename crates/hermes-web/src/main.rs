use hermes_core::GatewayConfig;
use hermes_web::{start_dashboard, DashboardConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let api_key = std::env::var("OPENAI_API_KEY")
        .or_else(|_| std::env::var("DEEPSEEK_API_KEY"))
        .unwrap_or_default();

    let base_url = std::env::var("OPENAI_BASE_URL")
        .or_else(|_| std::env::var("DEEPSEEK_BASE_URL"))
        .unwrap_or_else(|_| "https://api.deepseek.com".to_string());

    let config = DashboardConfig {
        port: std::env::var("PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(9090),
        gateway_config: GatewayConfig {
            api_key,
            base_url: Some(base_url),
            ..Default::default()
        },
        ..Default::default()
    };

    println!("🔷 Hermes Dashboard starting on :{}...", config.port);
    start_dashboard(config).await
}
