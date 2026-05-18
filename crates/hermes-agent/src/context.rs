use hermes_core::Conversation;

/// Manages conversation context — truncation, hygiene, compression
pub struct ContextEngine {
    pub max_messages: usize,
    pub max_context_chars: usize,
    pub protect_first_n: usize,
    pub protect_last_n: usize,
}

impl Default for ContextEngine {
    fn default() -> Self {
        Self {
            max_messages: 200,
            max_context_chars: 100_000,
            protect_first_n: 3,
            protect_last_n: 20,
        }
    }
}

impl ContextEngine {
    pub fn new() -> Self { Self::default() }

    /// Truncate conversation to stay within limits
    pub fn enforce(&self, conv: &mut Conversation) {
        if conv.context_length() <= self.max_context_chars
            && conv.messages.len() <= self.max_messages {
            return;
        }

        // Keep system messages (first N) and recent messages (last N)
        let total = conv.messages.len();
        if total <= self.protect_first_n + self.protect_last_n {
            return;
        }

        // Remove middle messages that are user role
        let safe_zone_start = self.protect_first_n;
        let safe_zone_end = total.saturating_sub(self.protect_last_n);
        if safe_zone_end <= safe_zone_start {
            return;
        }

        conv.messages.drain(safe_zone_start..safe_zone_end);
    }
}
