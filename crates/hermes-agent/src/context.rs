use hermes_core::Conversation;

/// Manages context window within token limits
pub struct ContextManager {
    max_tokens: usize,
}

impl ContextManager {
    pub fn new(max_tokens: usize) -> Self {
        Self { max_tokens }
    }

    pub fn trim(&self, conv: &mut Conversation, reserve_tokens: usize) {
        let limit = self.max_tokens.saturating_sub(reserve_tokens);
        conv.truncate(limit);
    }

    pub fn summarize(&self, conv: &Conversation) -> String {
        let total = conv.context_length();
        let msg_count = conv.messages.len();
        format!(
            "Context: {} messages, ~{} chars. Max: {} tokens",
            msg_count, total, self.max_tokens
        )
    }
}
