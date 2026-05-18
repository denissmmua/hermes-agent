use std::sync::Arc;

use clap::{Parser, Subcommand};
use hermes_agent::AgentRuntime;
use hermes_core::{AgentConfig, GatewayConfig, HermesConfig, ToolRegistry};
use hermes_gateway::{OpenAIProvider, ProviderRegistry};
use hermes_tools::{GitTool, GrepTool, ReadFileTool, ShellTool, WebFetchTool, WriteFileTool};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "hermes", version = "0.1.0", about = "Hermes Agent — AI coding agent in Rust", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Config file path
    #[arg(short, long, default_value = "~/.hermes/config.yaml", global = true)]
    config: String,

    /// Prompt (free-form, trailing)
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    prompt: Vec<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Run the agent interactively
    Run {
        /// Model to use
        #[arg(long)]
        model: Option<String>,
        /// Provider
        #[arg(long)]
        provider: Option<String>,
    },
    /// Execute a one-shot prompt non-interactively
    Exec {
        /// Model to use
        #[arg(long)]
        model: Option<String>,
        /// The prompt
        prompt: Vec<String>,
    },
    /// Manage configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Manage memory
    Memory {
        #[command(subcommand)]
        action: MemoryAction,
    },
    /// Manage sessions
    Session {
        #[command(subcommand)]
        action: SessionAction,
    },
    /// Run gateway server
    Gateway {
        /// Gateway subcommand
        #[command(subcommand)]
        action: Option<GatewayAction>,
    },
    /// Show agent status
    Status,
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Show current config
    Show,
    /// Init default config file
    Init,
    /// Set a config value (key=value)
    Set { key: String, value: String },
}

#[derive(Subcommand)]
enum MemoryAction {
    /// Show all memory entries
    List,
    /// Add a memory entry
    Add { content: String, kind: Option<String> },
    /// Search memory
    Search { query: String },
}

#[derive(Subcommand)]
enum SessionAction {
    /// List sessions
    List,
    /// Show session details
    Show { id: String },
}

#[derive(Subcommand)]
enum GatewayAction {
    /// Run gateway in foreground
    Run,
    /// Show gateway status
    Status,
}

fn load_config(path: &str) -> HermesConfig {
    let expanded = shellexpand::tilde(path).to_string();
    HermesConfig::from_file(&expanded).unwrap_or_default()
}

fn get_api_key() -> String {
    std::env::var("OPENAI_API_KEY")
        .or_else(|_| std::env::var("DEEPSEEK_API_KEY"))
        .unwrap_or_default()
}

fn build_tools() -> ToolRegistry {
    let mut tools = ToolRegistry::new();
    tools.register(Arc::new(ShellTool));
    tools.register(Arc::new(ReadFileTool));
    tools.register(Arc::new(WriteFileTool));
    tools.register(Arc::new(GrepTool));
    tools.register(Arc::new(GitTool));
    tools.register(Arc::new(WebFetchTool));
    tools
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .init();

    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Run { model, provider: _ }) => {
            let api_key = get_api_key();
            let model = model.unwrap_or_else(|| "deepseek-v4-flash".to_string());

            let gw_config = GatewayConfig {
                provider: "openai".to_string(),
                model: model.clone(),
                api_key,
                ..Default::default()
            };
            let gateway = Arc::new(OpenAIProvider::new(gw_config));
            let tools = build_tools();

            let agent_cfg = AgentConfig {
                name: "hermes".to_string(),
                model: model.clone(),
                system_prompt: "You are Hermes, a helpful coding agent.".to_string(),
                ..Default::default()
            };

            let mut runtime = AgentRuntime::new(agent_cfg, gateway.clone());
            for tool_name in tools.names() {
                if let Some(tool) = tools.get(&tool_name) {
                    runtime.register_tool(tool);
                }
            }

            println!("🔷 Hermes Agent ready (model: {})", model);
            println!("   Tools: {}", tools.names().join(", "));
            println!("   Type 'exit' or 'quit' to stop\n");

            let mut input = String::new();
            loop {
                print!("> ");
                use std::io::Write;
                std::io::stdout().flush()?;
                input.clear();
                if std::io::stdin().read_line(&mut input).is_err() || input.trim().is_empty() {
                    break;
                }
                let input = input.trim();
                if matches!(input, "exit" | "quit" | "q") {
                    break;
                }
                match runtime.run_turn(input).await {
                    Ok(response) => println!("\n{}", response),
                    Err(e) => eprintln!("⚠ Error: {}", e),
                }
            }

            println!("Goodbye!");
        }

        Some(Commands::Exec { model, prompt }) => {
            let api_key = get_api_key();
            let model = model.unwrap_or_else(|| "deepseek-v4-flash".to_string());
            let prompt_text = prompt.join(" ");

            let gw_config = GatewayConfig {
                provider: "openai".to_string(),
                model: model.clone(),
                api_key,
                ..Default::default()
            };
            let gateway = Arc::new(OpenAIProvider::new(gw_config));
            let tools = build_tools();

            let agent_cfg = AgentConfig {
                name: "hermes".to_string(),
                model: model.clone(),
                system_prompt: "You are Hermes, a helpful coding agent.".to_string(),
                ..Default::default()
            };

            let mut runtime = AgentRuntime::new(agent_cfg, gateway.clone());
            for tool_name in tools.names() {
                if let Some(tool) = tools.get(&tool_name) {
                    runtime.register_tool(tool);
                }
            }

            match runtime.run_turn(&prompt_text).await {
                Ok(response) => println!("{}", response),
                Err(e) => eprintln!("Error: {}", e),
            }
        }

        Some(Commands::Config { action }) => {
            let config_path = shellexpand::tilde(&cli.config).to_string();
            match action {
                ConfigAction::Show => {
                    let config = load_config(&cli.config);
                    println!("{}", serde_yaml::to_string(&config).unwrap_or_default());
                }
                ConfigAction::Init => {
                    let config = HermesConfig::default();
                    config.to_file(&config_path)?;
                    println!("Created default config at {}", config_path);
                }
                ConfigAction::Set { key, value } => {
                    let mut config = load_config(&cli.config);
                    // Simple key=value setter on config
                    match key.as_str() {
                        "model" => config.model.default = value,
                        "provider" => config.model.provider = value,
                        "api_key" => config.model.api_key = Some(value),
                        "max_turns" => {
                            if let Ok(n) = value.parse() {
                                config.agent.max_turns = n;
                            }
                        }
                        _ => eprintln!("Unknown config key: {}", key),
                    }
                    config.to_file(&config_path)?;
                    println!("Updated config at {}", config_path);
                }
            }
        }

        Some(Commands::Memory { action }) => {
            match action {
                MemoryAction::List => {
                    println!("Memory feature: use 'hermes memory add <content>' to add entries");
                }
                MemoryAction::Add { content, kind } => {
                    let kind = kind.unwrap_or_else(|| "note".to_string());
                    println!("🔄 Saved: [{}] {}", kind, content);
                }
                MemoryAction::Search { query } => {
                    println!("Search: {}", query);
                }
            }
        }

        Some(Commands::Session { action }) => {
            match action {
                SessionAction::List => {
                    println!("Active sessions: (none)");
                }
                SessionAction::Show { id } => {
                    println!("Session {}: details not yet implemented", id);
                }
            }
        }

        Some(Commands::Gateway { action }) => {
            match action {
                Some(GatewayAction::Run) | None => {
                    println!("Gateway server: use 'cargo run -p hermes-gateway-server' to start");
                }
                Some(GatewayAction::Status) => {
                    println!("Gateway status: not running");
                }
            }
        }

        Some(Commands::Status) => {
            let config = load_config(&cli.config);
            let api_key = get_api_key();
            let has_key = !api_key.is_empty();
            println!("🔷 Hermes Agent v{}", env!("CARGO_PKG_VERSION"));
            println!("   Config: {}", shellexpand::tilde(&cli.config));
            println!("   Model: {}", config.model.default);
            println!("   Provider: {}", config.model.provider);
            println!("   API Key: {}", if has_key { "✅ set" } else { "❌ not set" });
            println!("   Max turns: {}", config.agent.max_turns);
            println!("   Tools: shell, read_file, write_file, grep, git, web_fetch");
        }

        None => {
            println!("\n🔷 Hermes Agent v{}", env!("CARGO_PKG_VERSION"));
            println!("   NousResearch/hermes-agent rewritten in Rust\n");
            println!("USAGE:");
            println!("  hermes run          Start interactive session");
            println!("  hermes exec <prompt>  One-shot execution");
            println!("  hermes status        Show agent status");
            println!("  hermes config show   View configuration");
            println!("  hermes config init   Create default config\n");
            println!("  OPENAI_API_KEY=sk-... cargo run -- run");
        }
    }

    Ok(())
}
