use std::sync::Arc;
use clap::{Parser, Subcommand};
use hermes_agent::AgentRuntime;
use hermes_core::{AgentConfig, GatewayConfig, HermesConfig, ToolRegistry};
use std::path::PathBuf;
use hermes_gateway::OpenAIProvider;
use hermes_tools::{ShellTool, ReadFileTool, WriteFileTool, GrepTool, GitTool, WebFetchTool};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "hermes", version = "0.1.0", about = "Hermes Agent Rust")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
    #[arg(short, long, default_value = "~/.hermes/config.yaml", global = true)]
    config: String,
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    prompt: Vec<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Interactive session
    Run { #[arg(long)] model: Option<String> },
    /// One-shot execution
    Exec { #[arg(long)] model: Option<String>, prompt: Vec<String> },
    /// Save session to file
    Save { name: String },
    /// Load session from file
    Load { name: String },
    /// List sessions
    Sessions,
    /// Show config/skills/memory status
    Status,
    /// Manage config
    Config { #[command(subcommand)] action: ConfigAction },
    /// Gateway server
    Gateway,
    /// List available skills
    Skills,
}

#[derive(Subcommand)]
enum ConfigAction { Show, Init, Set { key: String, value: String }, Path }

fn api_key() -> String {
    std::env::var("OPENAI_API_KEY").or_else(|_| std::env::var("DEEPSEEK_API_KEY")).unwrap_or_default()
}
fn base_url() -> String {
    std::env::var("OPENAI_BASE_URL").or_else(|_| std::env::var("DEEPSEEK_BASE_URL"))
        .unwrap_or_else(|_| "https://api.deepseek.com".to_string())
}

fn load_gateway_config() -> GatewayConfig {
    // Try config file first
    let config_path = shellexpand::tilde("~/.hermes/config.yaml");
    if let Ok(content) = std::fs::read_to_string(config_path.as_ref()) {
        if let Ok(cfg) = serde_yaml::from_str::<HermesConfig>(&content) {
            // Check custom_providers for DeepSeek
            for cp in &cfg.custom_providers {
                if !cp.api_key.is_empty() {
                    return GatewayConfig {
                        model: cp.model.clone(),
                        api_key: cp.api_key.clone(),
                        base_url: Some(cp.base_url.clone()),
                        ..Default::default()
                    };
                }
            }
            // Fall back to model section
            if let Some(ak) = &cfg.model.api_key {
                if !ak.is_empty() {
                    let base = match cfg.model.base_url.as_ref() {
                        Some(u) if !u.is_empty() => Some(u.clone()),
                        _ => Some("https://api.deepseek.com".to_string()),
                    };
                    return GatewayConfig {
                        model: if cfg.model.default == "gpt-5.5" { "deepseek-chat".to_string() } else { cfg.model.default.clone() },
                        api_key: ak.clone(),
                        base_url: base,
                        ..Default::default()
                    };
                }
            }
        }
    }
    // Fall back to env vars
    GatewayConfig {
        model: "deepseek-chat".to_string(),
        api_key: api_key(),
        base_url: Some(base_url()),
        ..Default::default()
    }
}

fn build_runtime(model: &str) -> anyhow::Result<AgentRuntime> {
    let mut tools = ToolRegistry::new();
    let tool_list: Vec<Arc<dyn hermes_core::Tool>> = vec![
        Arc::new(ShellTool), Arc::new(ReadFileTool), Arc::new(WriteFileTool),
        Arc::new(GrepTool), Arc::new(GitTool), Arc::new(WebFetchTool),
    ];
    for t in tool_list { tools.register(t); }

    let mut gw_cfg = load_gateway_config();
    if !model.is_empty() && model != "deepseek-chat" {
        gw_cfg.model = model.to_string();
    }
    // If still no model, set default
    if gw_cfg.model.is_empty() { gw_cfg.model = "deepseek-chat".to_string(); }

    let gw = Arc::new(OpenAIProvider::new(gw_cfg));
    let mut rt = AgentRuntime::new(AgentConfig {
        name: "hermes".to_string(),
        model: model.to_string(),
        system_prompt: "You are Hermes, an AI coding agent in Rust. You have access to tools. Use them to complete tasks.".to_string(),
        ..Default::default()
    }, gw);
    for n in tools.names() {
        if let Some(t) = tools.get(&n) { rt.register_tool(t); }
    }
    Ok(rt)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::WARN.into())).init();
    let cli = Cli::parse();
    match cli.command {
        Some(Commands::Run { model }) => {
            let m = model.unwrap_or_else(|| "deepseek-chat".to_string());
            let mut rt = build_runtime(&m)?;
            println!("Hermes ({}) — type exit to quit", m);
            let mut input = String::new();
            loop {
                print!("> "); use std::io::Write; std::io::stdout().flush()?;
                input.clear();
                if std::io::stdin().read_line(&mut input).is_err() || input.trim().is_empty() { break; }
                let i = input.trim();
                if i.eq("exit") || i.eq("quit") || i.eq("q") { break; }
                match rt.run_turn(i).await { Ok(r) => println!("{}", r), Err(e) => eprintln!("Error: {}", e) }
            }
        }
        Some(Commands::Exec { model, prompt }) => {
            let m = model.unwrap_or_else(|| "deepseek-chat".to_string());
            let text = prompt.join(" ");
            if text.is_empty() { eprintln!("No prompt"); return Ok(()); }
            let mut rt = build_runtime(&m)?;
            match rt.run_turn(&text).await {
                Ok(r) => println!("{}", r),
                Err(e) => eprintln!("Error: {}", e),
            }
        }
        Some(Commands::Save { name }) => {
            let path = format!("/root/sessions/{}.json", name);
            std::fs::create_dir_all("/root/sessions").ok();
            let cfg = HermesConfig::default();
            let data = serde_json::to_string_pretty(&cfg)?;
            std::fs::write(&path, &data)?;
            println!("Saved: {}", path);
        }
        Some(Commands::Load { name }) => {
            let path = format!("/root/sessions/{}.json", name);
            match std::fs::read_to_string(&path) {
                Ok(data) => { let _: HermesConfig = serde_json::from_str(&data)?; println!("Loaded: {}", path); }
                Err(e) => eprintln!("Error: {}", e),
            }
        }
        Some(Commands::Sessions) => {
            let dir = PathBuf::from("/root/sessions");
            if dir.exists() {
                for e in std::fs::read_dir(&dir).unwrap() {
                    if let Ok(e) = e { println!("  {}", e.file_name().to_string_lossy()); }
                }
            } else { println!("No sessions"); }
        }
        Some(Commands::Status) => {
            println!("Hermes Agent Rust v{}", env!("CARGO_PKG_VERSION"));
            println!("  Endpoint: {}", base_url());
            println!("  API Key: {}", if !api_key().is_empty() { "set" } else { "missing" });
            println!("  Tools: shell, file, grep, git, web");
            println!("  Skills: coding, forex, system");
        }
        Some(Commands::Skills) => {
            println!("Skills:");
            for s in ["coding", "forex", "system"] {
                let path = format!("crates/hermes-core/skills/{}/SKILL.md", s);
                if let Ok(content) = std::fs::read_to_string(&path) {
                    let desc = content.lines().next().unwrap_or("").trim();
                    println!("  {}: {}", s, desc);
                }
            }
        }
        Some(Commands::Config { action }) => {
            let path = shellexpand::tilde(&cli.config).to_string();
            match action {
                ConfigAction::Show => {
                    match std::fs::read_to_string(&path) {
                        Ok(c) => println!("{}", c),
                        Err(_) => println!("No config at {}", path),
                    }
                }
                ConfigAction::Init => {
                    let cfg = HermesConfig::default();
                    cfg.to_file(&path).map_err(|e| anyhow::anyhow!("{}", e))?;
                    println!("Created: {}", path);
                }
                ConfigAction::Set { key, value } => {
                    println!("Set {} = {}", key, value);
                }
                ConfigAction::Path => { println!("{}", path); }
            }
        }
        Some(Commands::Gateway) => {
            println!("Gateway server: cargo run -p hermes-gateway-server");
        }
        None => {
            println!("Hermes Agent Rust v{}", env!("CARGO_PKG_VERSION"));
            println!("Commands:");
            println!("  run          Interactive session");
            println!("  exec <text>  One-shot execution");
            println!("  save <name>  Save session");
            println!("  load <name>  Load session");
            println!("  sessions     List sessions");
            println!("  skills       List skills");
            println!("  config       Config management");
            println!("  status       Show status");
        }
    }
    Ok(())
}
