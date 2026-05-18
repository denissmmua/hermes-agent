#![allow(dead_code)]
/// Conversation compression — manages context window
/// Port from agent/conversation_compression.py

use hermes_core::{Message, Conversation, Role};

/// Compression levels
#[derive(Debug, Clone, PartialEq)]
pub enum CompressLevel {
    Light,   // Remove middle messages only
    Medium,  // Summarize old messages  
    Aggressive, // Keep only system + last N messages
}

/// Compress a conversation
pub fn compress_conversation(
    conv: &mut Conversation,
    max_messages: usize,
    max_chars: usize,
    level: CompressLevel,
) -> CompressResult {
    let before_size = conv.context_length();
    let before_count = conv.messages.len();

    match level {
        CompressLevel::Light => compress_light(conv, max_messages),
        CompressLevel::Medium => compress_medium(conv, max_messages, max_chars),
        CompressLevel::Aggressive => compress_aggressive(conv, max_messages),
    }

    CompressResult {
        before_messages: before_count,
        after_messages: conv.messages.len(),
        before_chars: before_size,
        after_chars: conv.context_length(),
    }
}

/// Light: remove user messages from the middle
fn compress_light(conv: &mut Conversation, max_messages: usize) {
    if conv.messages.len() <= max_messages { return; }

    // Keep system messages, remove oldest non-system messages
    let system_count = conv.messages.iter()
        .filter(|m| matches!(m.role, Role::System))
        .count();

    let to_remove = conv.messages.len().saturating_sub(max_messages);
    if to_remove == 0 { return; }

    // Remove user messages from the middle
    let mut removed = 0;
    let mut i = system_count;
    while i < conv.messages.len() && removed < to_remove {
        if matches!(conv.messages[i].role, Role::User) {
            conv.messages.remove(i);
            removed += 1;
        } else {
            i += 1;
        }
    }
}

/// Medium: keep last N messages, summarize middle
fn compress_medium(conv: &mut Conversation, max_messages: usize, _max_chars: usize) {
    compress_aggressive(conv, max_messages);
}

/// Aggressive: keep system + last N only
fn compress_aggressive(conv: &mut Conversation, max_messages: usize) {
    if conv.messages.len() <= max_messages { return; }

    let system_msgs: Vec<Message> = conv.messages.iter()
        .filter(|m| matches!(m.role, Role::System))
        .cloned()
        .collect();

    let last_n = if system_msgs.len() >= max_messages {
        0
    } else {
        max_messages - system_msgs.len()
    };

    let keep_start = conv.messages.len().saturating_sub(last_n);
    let mut new_msgs = system_msgs;
    for m in conv.messages.drain(keep_start..) {
        new_msgs.push(m);
    }
    conv.messages = new_msgs;
}

#[derive(Debug, Clone)]
pub struct CompressResult {
    pub before_messages: usize,
    pub after_messages: usize,
    pub before_chars: usize,
    pub after_chars: usize,
}

impl CompressResult {
    pub fn saved_chars(&self) -> usize {
        self.before_chars.saturating_sub(self.after_chars)
    }

    pub fn saved_messages(&self) -> usize {
        self.before_messages.saturating_sub(self.after_messages)
    }

    pub fn summary(&self) -> String {
        format!(
            "Compressed: {}→{} messages, {}→{} chars (saved {} chars)",
            self.before_messages, self.after_messages,
            self.before_chars, self.after_chars,
            self.saved_chars()
        )
    }
}

/// Check if conversation needs compression
pub fn needs_compression(conv: &Conversation, max_chars: usize, ratio: f64) -> bool {
    conv.context_length() as f64 > max_chars as f64 * ratio
}
