// 1:1 port of trajectory_compressor.py - conversation compression/context management
use hermes_core::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionConfig {
    pub enabled: bool,
    pub threshold: f64,
    pub target_ratio: f64,
    pub protect_last_n: usize,
    pub protect_first_n: usize,
    pub min_message_len: usize,
    pub max_message_len: usize,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            enabled: true, threshold: 0.5, target_ratio: 0.2,
            protect_last_n: 20, protect_first_n: 3,
            min_message_len: 20, max_message_len: 500,
        }
    }
}

pub struct ContextCompressor {
    config: CompressionConfig,
}

impl ContextCompressor {
    pub fn new(config: CompressionConfig) -> Self { Self { config } }

    /// Estimate token count from text length
    pub fn estimate_tokens(text: &str) -> usize {
        (text.len() as f64 * 0.4) as usize + 1
    }

    /// Check if compression is needed based on context size vs limits
    pub fn needs_compression(&self, total_tokens: usize, max_tokens: usize) -> bool {
        let ratio = total_tokens as f64 / max_tokens as f64;
        ratio > self.config.threshold
    }

    /// Compress a conversation by summarizing older messages
    pub fn compress(&self, conv: &mut Conversation) -> CompressResult {
        let original_len = conv.messages.len();
        let original_tokens = conv.context_length();

        if original_len <= self.config.protect_first_n + self.config.protect_last_n {
            return CompressResult { original_tokens, compressed_tokens: original_tokens, removed: 0, summary: None };
        }

        // Keep first N and last M messages, summarize the middle
        let keep_start = self.config.protect_first_n;
        let keep_end = self.config.protect_last_n;

        let middle: Vec<Message> = conv.messages
            .drain(keep_start..original_len.saturating_sub(keep_end))
            .collect();

        let middle_text: Vec<String> = middle.iter()
            .filter(|m| m.content.len() > self.config.min_message_len)
            .map(|m| {
                let role = match m.role { Role::User => "User", _ => "Assistant" };
                format!("{}: {}",
                    role,
                    if m.content.len() > self.config.max_message_len {
                        format!("{}...", &m.content[..self.config.max_message_len])
                    } else { m.content.clone() }
                )
            })
            .collect();

        let summary = if !middle_text.is_empty() {
            let n = middle_text.len();
            let avg_len: usize = middle_text.iter().map(|s| s.len()).sum::<usize>() / n.max(1);
            Some(format!("[Compressed: {} messages, ~{} chars average]", n, avg_len))
        } else { None };

        if let Some(ref s) = summary {
            conv.push(Message::assistant(s));
        }

        CompressResult {
            original_tokens,
            compressed_tokens: conv.context_length(),
            removed: middle.len(),
            summary,
        }
    }

    /// Compress a single long message by truncation
    pub fn truncate_message(&self, text: &str, max_len: usize) -> String {
        if text.len() > max_len {
            format!("{}...\n[truncated {} chars]", &text[..max_len], text.len() - max_len)
        } else { text.to_string() }
    }
}

#[derive(Debug)]
pub struct CompressResult {
    pub original_tokens: usize,
    pub compressed_tokens: usize,
    pub removed: usize,
    pub summary: Option<String>,
}
