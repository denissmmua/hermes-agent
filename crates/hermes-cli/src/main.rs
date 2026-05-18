use std::sync::Arc;

use clap::{Parser, Subcommand};
use hermes_agent::AgentRuntime;
use hermes_core::{AgentConfig, GatewayConfig, HermesConfig, ToolRegistry};
use hermes_gateway::OpenAIProvider;
use hermes_tools::{GitTool, GrepTool, ReadFileTool, ShellTool, WebFetchTool, WriteFileTool};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "hermes", version = "0.1.0", about = "Hermes Agent — Rust port")]
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
    Run { #[arg(long)] model: Option<String> },
    Exec { #[arg(long)] model: Option<String>, prompt: Vec<String> },
    Config { #[command(subcommand)] action: ConfigAction },
    Status,
}

#[derive(Subcommand)]
enum ConfigAction { Show, Init, Set { key: String, value: String } }

fn get_api_key() -> String {
    std::env::var("OPENAI_API_KEY").or_else(|_| std::env::var("DEEPSEEK_API_KEY")).unwrap_or_default()
}

fn get_base_url() -> String {
    std::env::var("OPENAI_BASE_URL").or_else(|_| std::env::var("DEEPSEEK_BASE_URL"))
        .unwrap_or_else(|_| "https://api.deepseek.com".to_string())
}

fn build_runtime(model: String) -> anyhow::Result<AgentRuntime> {
    let mut tools = ToolRegistry::new();
    let toolbox: Vec<Arc<dyn hermes_core::Tool>> = vec![
        Arc::new(ShellTool), Arc::new(ReadFileTool), Arc::new(WriteFileTool),
        Arc::new(GrepTool), Arc::new(GitTool), Arc::new(WebFetchTool),
    ];
    for t in toolbox { tools.register(t); }

    let gateway = Arc::new(OpenAIProvider::new(GatewayConfig {
        model: model.clone(), api_key: get_api_key(),
        base_url: Some(get_base_url()), ..Default::default()
    }));

    let mut rt = AgentRuntime::new(AgentConfig {
        name: "hermes".to_string(), model: model.clone(),
        system_prompt: format!("\
You are Hermes, an AI coding agent rewritten in Rust (port of NousResearch/hermes-agent). \
You have tools for shell, file read/write, grep, git, and web fetching. \
Use native function calling when you need to execute commands or access files. \
Be concise and accurate."),
        ..Default::default()
    }, gateway.clone());

    for name in tools.names() {
        if let Some(t) = tools.get(&name) { rt.register_tool(t); }
    }
    Ok(rt)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::WARN.into()))
        .init();

    match Cli::parse().command {
        Some(Commands::Run { model }) => {
            let model = model.unwrap_or_else(|| "deepseek-chat".to_string());
            let mut rt = build_runtime(model.clone())?;
            println!("🔷 Hermes Agent Rust — {} @ {}", model, get_base_url());
            println!("   Tools: {}", rt.state().tools.names().join(", "));
            let mut input = String::new();
            loop {
                print!("\n> ");
                use std::io::Write; std::io::stdout().flush()?;
                input.clear();
                if std::io::stdin().read_line(&mut input).is_err() || input.trim().is_empty() { break; }
                let input = input.trim();
                if matches!(input, "exit" | "quit" | "q") { break; }
                match rt.run_turn(input).await {
                    Ok(r) => println!("\n{}", r),
                    Err(e) => eprintln!("⚠ {}", e),
                }
            }
        }

        Some(Commands::Exec { model, prompt }) => {
            let model = model.unwrap_or_else(|| "deepseek-chat".to_string());
            let prompt_text = prompt.join(" ");
            if prompt_text.is_empty() { eprintln!("Error: no prompt"); return Ok(()); }
            let mut rt = build_runtime(model.clone())?;
            match rt.run_turn(&prompt_text).await {
                Ok(r) => {
                    if !r.is_empty() { println!("{}", r); }
                    // Show tool execution results from conversation
                    let msgs: Vec<_> = rt.state().conversation.messages.iter()
                        .filter(|m| m.content.starts_with("Tool "))
                        .collect();
                    for m in &msgs { println!("{}", m.content); }
                }
                Err(e) => eprintln!("Error: {}", e),
            }
        }

        Some(Commands::Status) => {
            println!("🔷 Hermes Agent Rust v{}", env!("CARGO_PKG_VERSION"));
            println!("   Endpoint: {}", get_base_url());
            println!("   API Key: {}", if !get_api_key().is_empty() { "✅" } else { "❌" });
            println!("   Tools: shell, read_file, write_file, grep, git, web_fetch");
            println!("   Function calling: ✅ (native OpenAI format)");
        }

        _ => {
            println!("\n🔷 Hermes Agent Rust v{}", env!("CARGO_PKG_VERSION"));
            println!("   Port of NousResearch/hermes-agent\n");
            println!("USAGE:");
            println!("  hermes run            Interactive");
            println!("  hermes exec <prompt>  One-shot");
            println!("  hermes status         Status\n");
            println!("  OPENAI_API_KEY=sk-... cargo run -- run");
        }
    }
    Ok(())
}
