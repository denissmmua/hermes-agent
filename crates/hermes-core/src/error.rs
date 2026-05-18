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

#[cfg(test)]
mod tests {
    use crate::AgentError;

    #[test]
    fn test_error_display() {
        let e = AgentError::Provider("API down".into());
        assert!(e.to_string().contains("API down"));
    }

    #[test]
    fn test_rate_limited() {
        let e = AgentError::RateLimited(30);
        assert!(e.to_string().contains("30"));
    }
}
