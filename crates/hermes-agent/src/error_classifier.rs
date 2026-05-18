/// API error classification — maps error messages to actionable categories
#[derive(Debug, Clone, PartialEq)]
pub enum ApiErrorKind {
    RateLimited,         // 429
    AuthFailure,         // 401
    Timeout,             // connection timeout
    ServerError,         // 500/502/503
    ContextOverflow,     // context_length_exceeded
    ContentFilter,       // content policy violation
    InvalidRequest,      // 400 bad request
    ModelNotFound,       // 404 model not found
    Unknown(String),     // unclassified
}

impl ApiErrorKind {
    pub fn classify(error: &str) -> Self {
        let e = error.to_lowercase();
        if e.contains("rate") || e.contains("429") || e.contains("too many requests") {
            Self::RateLimited
        } else if e.contains("401") || e.contains("unauthorized") || e.contains("invalid_api_key")
            || e.contains("auth") || e.contains("authentication") {
            Self::AuthFailure
        } else if e.contains("timeout") || e.contains("timed out") || e.contains("deadline") {
            Self::Timeout
        } else if e.contains("503") || e.contains("502") || e.contains("500")
            || e.contains("service unavailable") || e.contains("server error") {
            Self::ServerError
        } else if e.contains("context_length_exceeded") || e.contains("max_tokens")
            || e.contains("context window") || e.contains("token limit") {
            Self::ContextOverflow
        } else if e.contains("content_filter") || e.contains("content_policy")
            || e.contains("safety") || e.contains("harmful") {
            Self::ContentFilter
        } else if e.contains("400") || e.contains("bad request") || e.contains("invalid")
            || e.contains("parse error") {
            Self::InvalidRequest
        } else if e.contains("404") || e.contains("not found") || e.contains("model")
            || e.contains("does not exist") {
            Self::ModelNotFound
        } else {
            Self::Unknown(error.to_string())
        }
    }

    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::RateLimited | Self::Timeout | Self::ServerError)
    }

    pub fn is_fatal(&self) -> bool {
        matches!(self, Self::AuthFailure | Self::ContentFilter | Self::ModelNotFound)
    }

    pub fn hint(&self) -> &'static str {
        match self {
            Self::RateLimited => "Wait and retry, or switch providers",
            Self::AuthFailure => "Check API key, base URL, and provider settings",
            Self::Timeout => "Increase timeout or use a faster model",
            Self::ServerError => "Provider may be down — retry or switch",
            Self::ContextOverflow => "Conversation too long — compress or reset",
            Self::ContentFilter => "Content was rejected by safety filters",
            Self::InvalidRequest => "Check request format and parameters",
            Self::ModelNotFound => "Check model name — it may not exist or be deprecated",
            Self::Unknown(_) => "Unknown error — check logs",
        }
    }
}

impl std::fmt::Display for ApiErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RateLimited => write!(f, "Rate Limited"),
            Self::AuthFailure => write!(f, "Auth Failure"),
            Self::Timeout => write!(f, "Timeout"),
            Self::ServerError => write!(f, "Server Error"),
            Self::ContextOverflow => write!(f, "Context Overflow"),
            Self::ContentFilter => write!(f, "Content Filter"),
            Self::InvalidRequest => write!(f, "Invalid Request"),
            Self::ModelNotFound => write!(f, "Model Not Found"),
            Self::Unknown(m) => write!(f, "Unknown: {}", m),
        }
    }
}

/// Parse available output tokens from a context overflow error
pub fn parse_context_limit_from_error(error: &str) -> Option<u32> {
    let re = regex_lite::Regex::new(r"(?:max|limit|context).*?(\d+)").ok()?;
    re.captures(error)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse().ok())
}

/// Parse how many tokens were requested from an overflow error
pub fn parse_requested_tokens_from_error(error: &str) -> Option<u32> {
    let re = regex_lite::Regex::new(r"you requested (\d+)").ok()?;
    re.captures(error)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse().ok())
}
