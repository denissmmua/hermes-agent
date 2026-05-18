use std::io::Write;
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use hermes_core::{
    AgentConfig, AgentError, GatewayConfig, HermesConfig, ToolRegistry,
    Conversation, Message,
};
use hermes_agent::AgentRuntime;
use hermes_gateway::OpenAIProvider;
use hermes_tools::{
    ShellTool, ReadFileTool, WriteFileTool, GrepTool, GitTool, WebFetchTool,
};

/// Load gateway config from config file, falling back to env vars
pub fn load_gateway_config() -> GatewayConfig {
    let config_path = shellexpand::tilde("~/.hermes/config.yaml");

    // Try config file first
    if let Ok(c) = std::fs::read_to_string(config_path.as_ref()) {
        if let Ok(cfg) = serde_yaml::from_str::<HermesConfig>(&c) {
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
            if let Some(ak) = &cfg.model.api_key {
                if !ak.is_empty() {
                    let base = match cfg.model.base_url.as_ref() {
                        Some(u) if !u.is_empty() => Some(u.clone()),
                        _ => Some("https://api.deepseek.com".to_string()),
                    };
                    let model = if cfg.model.default == "gpt-5.5" {
                        "deepseek-chat".to_string()
                    } else {
                        cfg.model.default.clone()
                    };
                    return GatewayConfig {
                        model,
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
        model: std::env::var("HERMES_MODEL").unwrap_or_else(|_| "deepseek-chat".to_string()),
        api_key: std::env::var("OPENAI_API_KEY")
            .or_else(|_| std::env::var("DEEPSEEK_API_KEY"))
            .unwrap_or_default(),
        base_url: Some(
            std::env::var("OPENAI_BASE_URL")
                .or_else(|_| std::env::var("DEEPSEEK_BASE_URL"))
                .unwrap_or_else(|_| "https://api.deepseek.com".to_string())
        ),
        ..Default::default()
    }
}

/// Build a full agent runtime with all tools
pub fn build_agent_runtime(model_override: Option<&str>) -> AgentRuntime {
    let mut gw_cfg = load_gateway_config();
    if let Some(m) = model_override {
        if !m.is_empty() {
            gw_cfg.model = m.to_string();
        }
    }
    let gateway = Arc::new(OpenAIProvider::new(gw_cfg.clone()));

    let mut tools = ToolRegistry::new();
    let tool_list: Vec<Arc<dyn hermes_core::Tool>> = vec![
        Arc::new(ShellTool),
        Arc::new(ReadFileTool),
        Arc::new(WriteFileTool),
        Arc::new(GrepTool),
        Arc::new(GitTool),
        Arc::new(WebFetchTool),
    ];
    for t in tool_list {
        tools.register(t);
    }

    let mut runtime = AgentRuntime::new(
        AgentConfig {
            name: "hermes".to_string(),
            model: gw_cfg.model.clone(),
            system_prompt: "\
You are Hermes, an AI coding agent in Rust.
You have access to these tools: shell, read_file, write_file, grep, git, web_fetch.
Use function calling (tool calls) when you need to perform actions.
Think step by step. Complete the task fully.".to_string(),
            ..Default::default()
        },
        gateway,
    );

    for name in tools.names() {
        if let Some(t) = tools.get(&name) {
            runtime.register_tool(t);
        }
    }

    runtime
}

// ═══════════════════════════════════════════════════════════════════════
// Hermes CLI – Full REPL
// ═══════════════════════════════════════════════════════════════════════

const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

#[derive(Debug, Clone)]
pub enum CliCommand {
    Help,
    Status,
    Model(String),
    Clear,
    Exit,
    Save(String),
    Load(String),
    Sessions,
    Skills,
    Config,
    History,
    Unknown(String),
}

impl CliCommand {
    pub fn parse(input: &str) -> Self {
        let trimmed = input.trim();
        if !trimmed.starts_with('/') {
            return CliCommand::Unknown(input.to_string());
        }
        let parts: Vec<&str> = trimmed.splitn(2, ' ').collect();
        let cmd = parts[0].to_lowercase();
        let arg = parts.get(1).unwrap_or(&"").trim().to_string();
        match cmd.as_str() {
            "/help" | "/h" | "/?" => CliCommand::Help,
            "/status" | "/st" => CliCommand::Status,
            "/model" | "/m" => CliCommand::Model(arg),
            "/clear" | "/c" | "/cls" => CliCommand::Clear,
            "/exit" | "/quit" | "/q" => CliCommand::Exit,
            "/save" => CliCommand::Save(arg),
            "/load" => CliCommand::Load(arg),
            "/sessions" => CliCommand::Sessions,
            "/skills" => CliCommand::Skills,
            "/config" => CliCommand::Config,
            "/history" => CliCommand::History,
            _ => CliCommand::Unknown(trimmed.to_string()),
        }
    }
}

/// Status bar info
#[derive(Debug, Clone)]
pub struct CliStatus {
    pub model: String,
    pub tokens_in: usize,
    pub tokens_out: usize,
    pub turn: u32,
    pub elapsed: String,
    pub agent_busy: bool,
    pub spinner_idx: usize,
    pub last_spinner_tick: Instant,
}

impl Default for CliStatus {
    fn default() -> Self {
        Self {
            model: "deepseek-chat".to_string(),
            tokens_in: 0,
            tokens_out: 0,
            turn: 0,
            elapsed: "0s".to_string(),
            agent_busy: false,
            spinner_idx: 0,
            last_spinner_tick: Instant::now(),
        }
    }
}

impl CliStatus {
    pub fn tick(&mut self) {
        if self.last_spinner_tick.elapsed() >= Duration::from_millis(100) {
            self.spinner_idx = (self.spinner_idx + 1) % SPINNER_FRAMES.len();
            self.last_spinner_tick = Instant::now();
        }
    }

    pub fn render(&self) -> String {
        let spinner = if self.agent_busy {
            SPINNER_FRAMES[self.spinner_idx]
        } else {
            " "
        };
        let turn_str = if self.turn > 0 {
            format!(" turn:{}", self.turn)
        } else {
            String::new()
        };
        format!(
            "{} {} | {} | {}{}",
            spinner,
            self.model,
            self.elapsed,
            self.tokens_in.saturating_add(self.tokens_out),
            turn_str
        )
    }
}

/// The main Hermes CLI application
pub struct HermesCli {
    pub runtime: AgentRuntime,
    pub model: String,
    pub status: CliStatus,
    pub history: Vec<String>,
    pub session_start: Instant,
    pub running: bool,
    pub max_turns: u32,
}

impl HermesCli {
    pub fn new(model_override: Option<&str>) -> Self {
        let runtime = build_agent_runtime(model_override);
        let model = runtime.state().config.model.clone();

        Self {
            model,
            status: CliStatus::default(),
            history: Vec::new(),
            session_start: Instant::now(),
            running: true,
            max_turns: 90,
            runtime,
        }
    }

    /// Format elapsed time
    fn fmt_elapsed(dur: Duration) -> String {
        let secs = dur.as_secs();
        if secs < 60 {
            format!("{}s", secs)
        } else if secs < 3600 {
            format!("{}m{}s", secs / 60, secs % 60)
        } else {
            format!("{}h{}m", secs / 3600, (secs % 3600) / 60)
        }
    }

    /// Update status bar from runtime state
    fn sync_status(&mut self) {
        let elapsed = self.session_start.elapsed();
        self.status.model = self.model.clone();
        self.status.turn = self.runtime.state().turn;
        self.status.elapsed = Self::fmt_elapsed(elapsed);
    }

    /// Show banner on startup
    pub fn show_banner(&self) {
        let ver = env!("CARGO_PKG_VERSION");
        let line = "─".repeat(50);
        println!("{}", line);
        println!("  🔷 Hermes Agent Rust v{}", ver);
        println!("  Model: {}", self.model);
        println!("  Tools: shell, read_file, write_file, grep, git, web_fetch");
        println!("  Type /help for commands");
        println!("{}", line);
    }

    /// Show help
    fn show_help(&self) {
        println!("╔══════════════════════════════════════╗");
        println!("║         Hermes CLI Commands          ║");
        println!("╠══════════════════════════════════════╣");
        println!("║ /help, /h        Show this help      ║");
        println!("║ /status, /st     Show agent status   ║");
        println!("║ /model <name>    Switch model        ║");
        println!("║ /clear, /c       Clear screen        ║");
        println!("║ /save <name>     Save session        ║");
        println!("║ /load <name>     Load session        ║");
        println!("║ /sessions        List sessions       ║");
        println!("║ /skills          List skills         ║");
        println!("║ /config          Show config         ║");
        println!("║ /history         Show session history║");
        println!("║ /exit, /quit     Exit CLI            ║");
        println!("╚══════════════════════════════════════╝");
    }

    /// Show status
    fn cmd_status(&mut self) {
        self.sync_status();
        let state = self.runtime.state();
        println!("── Status ────────────────────────────");
        println!("  Model:     {}", self.model);
        println!("  Turn:      {}", state.turn);
        println!("  Messages:  {}", state.conversation.messages.len());
        println!("  Tokens:    ~{}", state.conversation.context_length());
        println!("  Max turns: {}", self.max_turns);
        println!("  Uptime:    {}", self.status.elapsed);
        println!("────────────────────────────────────────");
    }

    /// Handle a slash command
    fn handle_command(&mut self, cmd: CliCommand) -> bool {
        match cmd {
            CliCommand::Help => { self.show_help(); false }
            CliCommand::Status => { self.cmd_status(); false }
            CliCommand::Model(name) => {
                if name.is_empty() {
                    println!("Current model: {}", self.model);
                    println!("Usage: /model <name>");
                } else {
                    self.model = name;
                    self.runtime = build_agent_runtime(Some(&self.model));
                    println!("Switched to model: {}", self.model);
                }
                false
            }
            CliCommand::Clear => {
                print!("\x1B[2J\x1B[H");
                std::io::stdout().flush().ok();
                false
            }
            CliCommand::Exit => {
                println!("Bye! 👋");
                true
            }
            CliCommand::Save(name) => {
                let path = format!("/root/sessions/{}.json", name);
                std::fs::create_dir_all("/root/sessions").ok();
                let state = self.runtime.state();
                if let Ok(json) = serde_json::to_string_pretty(&state.conversation) {
                    std::fs::write(&path, &json).ok();
                    println!("Session saved: {} ({} messages)", path, state.conversation.messages.len());
                } else {
                    eprintln!("Failed to serialise session");
                }
                false
            }
            CliCommand::Load(name) => {
                let path = format!("/root/sessions/{}.json", name);
                match std::fs::read_to_string(&path) {
                    Ok(data) => {
                        if let Ok(conv) = serde_json::from_str::<Conversation>(&data) {
                            self.runtime.state_mut().conversation = conv;
                            println!("Session loaded: {} ({} messages)", path, self.runtime.state().conversation.messages.len());
                        } else {
                            eprintln!("Failed to parse session");
                        }
                    }
                    Err(e) => eprintln!("Error: {}", e),
                }
                false
            }
            CliCommand::Sessions => {
                let dir = PathBuf::from("/root/sessions");
                if dir.exists() {
                    let entries: Vec<_> = std::fs::read_dir(&dir)
                        .into_iter()
                        .flatten()
                        .filter_map(|e| e.ok())
                        .collect();
                    if entries.is_empty() {
                        println!("No sessions found");
                    } else {
                        println!("Sessions:");
                        for e in entries {
                            println!("  {}", e.file_name().to_string_lossy());
                        }
                    }
                } else {
                    println!("No sessions directory");
                }
                false
            }
            CliCommand::Skills => {
                for s in ["coding", "forex", "system"] {
                    let path = format!("skills/{}/SKILL.md", s);
                    match std::fs::read_to_string(&path) {
                        Ok(content) => {
                            let first = content.lines().next().unwrap_or("");
                            println!("  {}: {}", s, first);
                        }
                        Err(_) => println!("  {}: (not loaded)", s),
                    }
                }
                false
            }
            CliCommand::Config => {
                let path = shellexpand::tilde("~/.hermes/config.yaml");
                match std::fs::read_to_string(path.as_ref()) {
                    Ok(c) => {
                        // Show just the model section
                        for line in c.lines() {
                            if line.starts_with("model:") || line.starts_with("  ") || line.starts_with("gateway:")
                                || line.starts_with("  api_key:") || line.starts_with("  base_url:")
                            {
                                let masked = if line.contains("api_key") {
                                    "  api_key: ***".to_string()
                                } else {
                                    line.to_string()
                                };
                                println!("{}", masked);
                            }
                        }
                    }
                    Err(_) => eprintln!("No config at {}", path),
                }
                false
            }
            CliCommand::History => {
                let count = self.history.len();
                if count == 0 {
                    println!("No history");
                } else {
                    println!("Last {} messages:", count);
                    for m in self.runtime.state().conversation.messages.iter() {
                        let role_str = match m.role {
                            hermes_core::Role::System => "sys",
                            hermes_core::Role::User => "usr",
                            hermes_core::Role::Assistant => "ast",
                            hermes_core::Role::Tool => "tls",
                        };
                        let content_preview: String = m.content.chars().take(80).collect();
                        println!("  [{}] {}", role_str, content_preview);
                    }
                }
                false
            }
            CliCommand::Unknown(s) => {
                if s.starts_with('/') {
                    println!("Unknown command: {}. Type /help", s);
                    false
                } else {
                    // Not a command, should be processed by agent
                    false
                }
            }
        }
    }

    /// Run one turn with the agent — observe-think-act loop
    pub async fn run_turn(&mut self, input: &str) {
        self.status.agent_busy = true;
        self.history.push(input.to_string());
        let turn_start = Instant::now();

        // Show spinner while thinking
        print!("\r{} Thinking...", SPINNER_FRAMES[0]);
        std::io::stdout().flush().ok();

        match self.runtime.run_turn(input).await {
            Ok(response) => {
                let elapsed = turn_start.elapsed();
                let elapsed_str = Self::fmt_elapsed(elapsed);

                // Clear spinner line
                print!("\r\x1B[K");
                std::io::stdout().flush().ok();

                // Show response
                println!("\n🤖 {}\n", response);

                self.status.turn = self.runtime.state().turn;
                self.status.elapsed = elapsed_str.clone();

                // Show tool calls summary
                let tool_count = self.runtime.state().conversation.messages.iter()
                    .filter(|m| matches!(m.role, hermes_core::Role::Tool))
                    .count();
                if tool_count > 0 {
                    println!("  ─ tools: {} calls in {}", tool_count, elapsed_str);
                } else {
                    println!("  ─ {}s", elapsed_str);
                }
            }
            Err(e) => {
                print!("\r\x1B[K");
                eprintln!("\n⚠ Error: {}\n", e);
            }
        }

        self.status.agent_busy = false;
    }

    /// Main REPL loop
    pub async fn repl(&mut self) {
        self.show_banner();

        let mut input = String::new();

        while self.running {
            // Prompt
            let prompt = format!("\x1B[36m({}) > \x1B[0m", self.model);
            print!("{}", prompt);
            std::io::stdout().flush().ok();

            input.clear();
            match std::io::stdin().read_line(&mut input) {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }

            let trimmed = input.trim();
            if trimmed.is_empty() {
                continue;
            }

            // Check for slash commands
            let cmd = CliCommand::parse(trimmed);
            let is_slash = trimmed.starts_with('/');

            if is_slash {
                let should_exit = self.handle_command(cmd);
                if should_exit {
                    break;
                }
            } else {
                // Run agent
                self.run_turn(trimmed).await;
            }
        }

        println!("\nSession closed after {}", Self::fmt_elapsed(self.session_start.elapsed()));
    }

    /// One-shot execution (non-interactive)
    pub async fn exec(&mut self, text: &str) -> String {
        self.status.agent_busy = true;
        match self.runtime.run_turn(text).await {
            Ok(response) => {
                self.status.agent_busy = false;
                response
            }
            Err(e) => {
                self.status.agent_busy = false;
                format!("Error: {}", e)
            }
        }
    }
}
