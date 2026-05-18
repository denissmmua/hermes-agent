// 1:1 port of hermes_constants.py
use std::path::PathBuf;

// ── Version ──
pub const VERSION: &str = "0.14.0";
pub const AGENT_NAME: &str = "hermes";

// ── Paths ──
pub fn default_config_dir() -> PathBuf {
    dirs_or_env("HERMES_CONFIG_DIR", ".hermes")
}
pub fn default_data_dir() -> PathBuf {
    dirs_or_env("HERMES_DATA_DIR", ".hermes/data")
}
pub fn default_state_dir() -> PathBuf {
    dirs_or_env("HERMES_STATE_DIR", ".hermes/state")
}
pub fn default_cache_dir() -> PathBuf {
    dirs_or_env("HERMES_CACHE_DIR", ".hermes/cache")
}
pub fn default_log_dir() -> PathBuf {
    dirs_or_env("HERMES_LOG_DIR", ".hermes/logs")
}
pub fn default_skills_dir() -> PathBuf {
    dirs_or_env("HERMES_SKILLS_DIR", ".hermes/skills")
}
pub fn default_tools_dir() -> PathBuf {
    dirs_or_env("HERMES_TOOLS_DIR", ".hermes/tools")
}
pub fn default_plugins_dir() -> PathBuf {
    dirs_or_env("HERMES_PLUGINS_DIR", ".hermes/plugins")
}
pub fn default_sessions_dir() -> PathBuf {
    dirs_or_env("HERMES_SESSIONS_DIR", ".hermes/sessions")
}

fn dirs_or_env(env: &str, default: &str) -> PathBuf {
    std::env::var(env)
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
            PathBuf::from(home).join(default)
        })
}

// ── Limits ──
pub const DEFAULT_MAX_TURNS: u32 = 90;
pub const DEFAULT_MAX_TOKENS: usize = 128_000;
pub const DEFAULT_MAX_RETRIES: u32 = 3;
pub const DEFAULT_TIMEOUT_SECS: u64 = 120;
pub const DEFAULT_GATEWAY_TIMEOUT: u64 = 1800;
pub const DEFAULT_CONTEXT_LIMIT: usize = 200_000;
pub const DEFAULT_MAX_TOOL_OUTPUT: usize = 50_000;
pub const DEFAULT_FILE_READ_MAX: usize = 100_000;

// ── Default Models ──
pub const DEFAULT_MODEL: &str = "deepseek-chat";
pub const DEFAULT_EMBEDDING_MODEL: &str = "text-embedding-3-small";

// ── Log Levels ──
pub const LOG_LEVELS: &[&str] = &["DEBUG", "INFO", "WARNING", "ERROR", "CRITICAL"];

// ── API Endpoints ──
pub const OPENAI_API: &str = "https://api.openai.com/v1";
pub const DEEPSEEK_API: &str = "https://api.deepseek.com/v1";
pub const ANTHROPIC_API: &str = "https://api.anthropic.com/v1";

// ── File Extensions ──
pub const CODE_EXTENSIONS: &[&str] = &[
    "rs", "py", "js", "ts", "go", "rb", "java", "c", "cpp", "h", "hpp",
    "sql", "sh", "bash", "zsh", "fish", "toml", "yaml", "yml", "json", "md",
];

// ── Exit Codes ──
pub const EXIT_SUCCESS: i32 = 0;
pub const EXIT_FAILURE: i32 = 1;
pub const EXIT_CONFIG_ERROR: i32 = 2;
pub const EXIT_API_ERROR: i32 = 3;
pub const EXIT_TIMEOUT: i32 = 4;
