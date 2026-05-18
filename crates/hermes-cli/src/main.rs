use clap::{Parser, Subcommand};
use std::path::PathBuf;
use hermes_cli::{HermesCli, load_gateway_config};

#[derive(Parser)]
#[command(name = "hermes", version = "0.1.0", about = "Hermes Agent Rust CLI")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    #[arg(short, long, global = true)]
    model: Option<String>,

    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    prompt: Vec<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Interactive REPL session
    Run,
    /// One-shot execution
    Exec { prompt: Vec<String> },
    /// Save session to file
    Save { name: String },
    /// Load session from file
    Load { name: String },
    /// List saved sessions
    Sessions,
    /// Show help / status
    Status,
    /// Manage config
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Show current config
    Show,
    /// Initialize default config
    Init,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Minimal logging — only warnings
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::WARN.into())
        )
        .init();

    let cli = Cli::parse();

    // Handle standalone commands that don't need a runtime
    match &cli.command {
        Some(Commands::Status) => {
            let gw = load_gateway_config();
            println!("Hermes Agent Rust v{}", env!("CARGO_PKG_VERSION"));
            println!("  Model:         {}", gw.model);
            println!("  Base URL:      {}", gw.base_url.unwrap_or_default());
            println!("  API Key:       {}", if gw.api_key.is_empty() { "missing" } else { "****" });
            println!("  Max retries:   {}", gw.max_retries);
            println!("  Timeout:       {}s", gw.timeout_secs);
            return Ok(());
        }
        Some(Commands::Sessions) => {
            let dir = PathBuf::from("/root/sessions");
            if dir.exists() {
                for e in std::fs::read_dir(&dir).unwrap() {
                    if let Ok(e) = e {
                        println!("  {}", e.file_name().to_string_lossy());
                    }
                }
            } else {
                println!("No sessions");
            }
            return Ok(());
        }
        Some(Commands::Config { action }) => {
            match action {
                ConfigAction::Show => {
                    let path = shellexpand::tilde("~/.hermes/config.yaml");
                    match std::fs::read_to_string(path.as_ref()) {
                        Ok(c) => println!("{}", c),
                        Err(_) => println!("No config at {}", path),
                    }
                }
                ConfigAction::Init => {
                    let path = shellexpand::tilde("~/.hermes/config.yaml");
                    let cfg = hermes_core::HermesConfig::default();
                    cfg.to_file(path.as_ref())
                        .map_err(|e| anyhow::anyhow!("{}", e))?;
                    println!("Created config at {}", path);
                }
            }
            return Ok(());
        }
        _ => {}
    }

    // Commands that need a runtime
    match cli.command {
        Some(Commands::Run) | None => {
            let mut cli_session = HermesCli::new(cli.model.as_deref());
            cli_session.repl().await;
        }
        Some(Commands::Exec { prompt }) => {
            let text = if !cli.prompt.is_empty() {
                cli.prompt.join(" ")
            } else if !prompt.is_empty() {
                prompt.join(" ")
            } else {
                eprintln!("No prompt provided. Usage: hermes exec \"your prompt\"");
                return Ok(());
            };
            let mut cli_session = HermesCli::new(cli.model.as_deref());
            let response = cli_session.exec(&text).await;
            println!("{}", response);
        }
        Some(Commands::Save { name }) => {
            let mut cli_session = HermesCli::new(cli.model.as_deref());
            let path = format!("/root/sessions/{}.json", name);
            std::fs::create_dir_all("/root/sessions").ok();
            let state = cli_session.runtime.state();
            if let Ok(json) = serde_json::to_string_pretty(&state.conversation) {
                std::fs::write(&path, &json).ok();
                println!("Session saved: {} ({} messages)", path, state.conversation.messages.len());
            }
        }
        Some(Commands::Load { name }) => {
            let path = format!("/root/sessions/{}.json", name);
            match std::fs::read_to_string(&path) {
                Ok(data) => {
                    if let Ok(conv) = serde_json::from_str::<hermes_core::Conversation>(&data) {
                        let mut cli_session = HermesCli::new(cli.model.as_deref());
                        cli_session.runtime.state_mut().conversation = conv;
                        println!("Session loaded: {} ({} messages)", path, cli_session.runtime.state().conversation.messages.len());
                        cli_session.repl().await;
                    } else {
                        eprintln!("Failed to parse session file");
                    }
                }
                Err(e) => eprintln!("Error: {}", e),
            }
        }
        _ => {}
    }

    Ok(())
}
