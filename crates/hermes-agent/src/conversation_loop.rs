use std::collections::HashMap;
use std::time::{Duration, Instant};

use hermes_core::{AgentError, AgentResult, Conversation, Message, Role, ToolInput};
use hermes_gateway::{LLMRequest, RetryConfig};
use rand::Rng;

use crate::ai_agent::{AIAgent, GuardrailVerdict};

// ═══════════════════════════════════════════════════════════════════════
// Retry utils — jittered backoff
// ═══════════════════════════════════════════════════════════════════════

/// Calculate jittered backoff delay
pub fn jittered_backoff(attempt: u32, base_ms: u64, max_ms: u64) -> Duration {
    let mut rng = rand::thread_rng();
    let exp = base_ms * 2u64.pow(attempt.saturating_sub(1));
    let capped = exp.min(max_ms);
    let jitter = rng.gen_range(0..=capped / 4);
    Duration::from_millis(capped + jitter)
}

// ═══════════════════════════════════════════════════════════════════════
// Error classifier
// ═══════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq)]
pub enum FailoverReason {
    RateLimited,
    AuthError,
    Timeout,
    ServerError,
    ContextLengthExceeded,
    BadRequest,
    Unknown,
}

impl FailoverReason {
    pub fn classify(error: &str) -> Self {
        let e = error.to_lowercase();
        if e.contains("rate") || e.contains("429") || e.contains("too many") {
            FailoverReason::RateLimited
        } else if e.contains("auth") || e.contains("401") || e.contains("unauthorized") || e.contains("key") {
            FailoverReason::AuthError
        } else if e.contains("timeout") || e.contains("timed out") {
            FailoverReason::Timeout
        } else if e.contains("503") || e.contains("502") || e.contains("service unavailable") {
            FailoverReason::ServerError
        } else if e.contains("context_length") || e.contains("max_tokens") || e.contains("token limit") {
            FailoverReason::ContextLengthExceeded
        } else if e.contains("400") || e.contains("bad request") || e.contains("invalid") {
            FailoverReason::BadRequest
        } else {
            FailoverReason::Unknown
        }
    }

    pub fn is_retryable(&self) -> bool {
        matches!(self, FailoverReason::RateLimited | FailoverReason::Timeout | FailoverReason::ServerError)
    }

    pub fn is_failover_reason(&self) -> bool {
        matches!(self, FailoverReason::RateLimited | FailoverReason::AuthError | FailoverReason::Timeout | FailoverReason::ServerError)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// System prompt builder
// ═══════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct SystemPromptBuilder {
    pub base_prompt: String,
    pub personality: Option<String>,
    pub platform_hints: String,
    pub tool_descriptions: String,
    pub skill_context: String,
}

impl Default for SystemPromptBuilder {
    fn default() -> Self {
        Self {
            base_prompt: "You are Hermes, an AI coding agent.".to_string(),
            personality: None,
            platform_hints: String::new(),
            tool_descriptions: String::new(),
            skill_context: String::new(),
        }
    }
}

impl SystemPromptBuilder {
    pub fn build(&self) -> String {
        let mut parts: Vec<&str> = Vec::new();
        parts.push(&self.base_prompt);

        if let Some(ref p) = self.personality {
            parts.push(p);
        }

        if !self.tool_descriptions.is_empty() {
            parts.push("Available tools:");
            parts.push(&self.tool_descriptions);
        }

        if !self.skill_context.is_empty() {
            parts.push(&self.skill_context);
        }

        if !self.platform_hints.is_empty() {
            parts.push(&self.platform_hints);
        }

        parts.join("\n\n")
    }

    pub fn with_personality(mut self, p: &str) -> Self {
        self.personality = Some(p.to_string());
        self
    }

    pub fn with_tools(mut self, desc: &str) -> Self {
        self.tool_descriptions = desc.to_string();
        self
    }

    pub fn with_platform(mut self, hint: &str) -> Self {
        self.platform_hints = hint.to_string();
        self
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Context compression trigger
// ═══════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct ContextManager {
    pub max_context_chars: usize,
    pub compression_threshold: f64,
    pub target_ratio: f64,
    pub min_messages_after_compress: usize,
}

impl Default for ContextManager {
    fn default() -> Self {
        Self {
            max_context_chars: 100_000,
            compression_threshold: 0.5,
            target_ratio: 0.2,
            min_messages_after_compress: 10,
        }
    }
}

impl ContextManager {
    pub fn new() -> Self { Self::default() }

    /// Check if context needs compression, return compressed messages if so
    pub fn maybe_compress(&self, conv: &Conversation) -> Option<Conversation> {
        let total = conv.context_length();
        if total as f64 > self.max_context_chars as f64 * self.compression_threshold {
            return Some(self.compress(conv));
        }
        None
    }

    /// Compress conversation: summarize old messages, keep recent ones
    pub fn compress(&self, conv: &Conversation) -> Conversation {
        let mut compressed = Conversation::new();

        // Keep system messages
        for m in &conv.messages {
            if matches!(m.role, Role::System) {
                compressed.push(m.clone());
            }
        }

        // Keep recent messages
        let keep_from = conv.messages.len().saturating_sub(self.min_messages_after_compress);
        for m in conv.messages.iter().skip(keep_from) {
            compressed.push(m.clone());
        }

        // Add a note about compression
        compressed.push(Message::system(format!(
            "[Context compressed: {} messages ({}) summarized for space]",
            keep_from,
            total_size(conv)
        )));

        compressed
    }
}

fn total_size(conv: &Conversation) -> String {
    let chars = conv.context_length();
    if chars < 1000 {
        format!("{} chars", chars)
    } else {
        format!("{:.1}K chars", chars as f64 / 1000.0)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Token counting
// ═══════════════════════════════════════════════════════════════════════

/// Rough token estimate (chars / ~4 for English, less reliable for other langs)
pub fn estimate_tokens(text: &str) -> u32 {
    (text.len() as f64 / 3.5) as u32
}

pub fn estimate_messages_tokens(messages: &[Message]) -> u32 {
    messages.iter().map(|m| estimate_tokens(&m.content) + 4).sum()
}

// ═══════════════════════════════════════════════════════════════════════
// Advanced run_turn with provider failover
// ═══════════════════════════════════════════════════════════════════════

impl AIAgent {
    /// Run a turn with automatic provider failover on errors
    pub async fn run_turn_with_failover(&mut self, input: &str) -> AgentResult<String> {
        let providers = self.provider_registry.fallback_order.clone();
        if providers.is_empty() {
            return self.run_turn(input).await;
        };

        // Try with primary first
        match self.run_turn(input).await {
            Ok(response) => return Ok(response),
            Err(e) => {
                let reason = FailoverReason::classify(&e.to_string());
                if !reason.is_retryable() && !reason.is_failover_reason() {
                    return Err(e);
                }
                eprintln!("⚠ Primary provider failed: {} -- trying fallback", e);
            }
        }

        // Try fallback providers
        for provider_name in providers.iter().skip(1) {
            let old_provider = self.active_provider_name.clone();
            self.active_provider_name = provider_name.clone();

            if self.provider_registry.get(provider_name).is_some() {
                // Remove tool result messages (they won't make sense with different provider)
                self.conversation.messages.retain(|m| !matches!(m.role, Role::Tool));

                match self.run_turn(input).await {
                    Ok(response) => return Ok(response),
                    Err(e) => {
                        let reason = FailoverReason::classify(&e.to_string());
                        if !reason.is_retryable() && !reason.is_failover_reason() {
                            self.active_provider_name = old_provider;
                            return Err(e);
                        }
                        eprintln!("⚠ Fallback {} failed: {}", provider_name, e);
                    }
                }
            }
        }

        Err(AgentError::Provider("All providers failed".into()))
    }
}
