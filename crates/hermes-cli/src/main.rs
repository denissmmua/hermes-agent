use std::sync::Arc;

use clap::{Parser, Subcommand};
use hermes_agent::AgentRuntime;
use hermes_core::{AgentConfig, GatewayConfig, ToolRegistry};
use hermes_gateway::OpenAIProvider;
use hermes_tools::{ReadFileTool, ShellTool, WriteFileTool};

#[derive(Parser)]
#[command(name = "hermes", version, about = "Hermes Agent — AI coding agent in Rust")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Prompt to run (trailing)
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    prompt: Vec<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Run the agent
    Run {
        /// Model to use
        #[arg(long, default_value = "deepseek-v4-flash")]
        model: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Run { model }) => {
            let api_key = std::env::var("OPENAI_API_KEY").ok();
            run_agent(&model, api_key.as_deref()).await?;
        }
        None => {
            println!("Hermes Agent (v{}). Use 'hermes run' to start.", env!("CARGO_PKG_VERSION"));
        }
    }
    Ok(())
}

async fn run_agent(model: &str, api_key: Option<&str>) -> anyhow::Result<()> {
    let gw_config = GatewayConfig {
        model: model.to_string(),
        api_key: api_key.unwrap_or("").to_string(),
        ..Default::default()
    };

    let gateway = Arc::new(OpenAIProvider::new(gw_config));
    let mut tools = ToolRegistry::new();
    tools.register(Arc::new(ShellTool));
    tools.register(Arc::new(ReadFileTool));
    tools.register(Arc::new(WriteFileTool));

    let agent_cfg = AgentConfig {
        name: "hermes".to_string(),
        model: model.to_string(),
        system_prompt: "You are Hermes, a helpful coding agent.".to_string(),
        ..Default::default()
    };

    let mut runtime = AgentRuntime::new(agent_cfg, gateway);
    *runtime.state_mut() = hermes_core::AgentState::new(
        AgentConfig {
            name: "hermes".to_string(),
            model: model.to_string(),
            system_prompt: "You are Hermes, a helpful coding agent.".to_string(),
            ..Default::default()
        },
        tools,
    );

    println!("Hermes Agent ready (model: {})", model);
    println!("Tools: {:?}", runtime.state().tools.names());

    let mut input = String::new();
    loop {
        print!("\n> ");
        use std::io::Write;
        std::io::stdout().flush()?;
        input.clear();
        if std::io::stdin().read_line(&mut input).is_err() || input.trim().is_empty() {
            break;
        }
        let input = input.trim();
        if input == "exit" || input == "quit" {
            break;
        }
        match runtime.run_turn(input).await {
            Ok(response) => println!("\n{}", response),
            Err(e) => eprintln!("Error: {}", e),
        }
    }
    Ok(())
}
