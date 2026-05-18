use thiserror::Error;

#[derive(Error, Debug)]
pub enum AgentError {
    #[error("LLM provider error: {0}")]
    Provider(String),

    #[error("Tool execution error: {0}")]
    Tool(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("State error: {0}")]
    State(String),

    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Rate limited. Retry after: {0}")]
    RateLimited(u64),

    #[error("Context length exceeded. Max: {0}, used: {1}")]
    ContextOverflow(usize, usize),

    #[error("Plugin error: {0}")]
    Plugin(String),

    #[error("Unknown error: {0}")]
    Unknown(String),
}

pub type AgentResult<T> = Result<T, AgentError>;
