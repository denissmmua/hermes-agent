#![allow(dead_code)]
use hermes_core::{Message, Role, ToolDefinition, Conversation};
use hermes_gateway::LLMMessage;

/// Sanitize messages: strip surrogates, limit content length
pub fn sanitize_messages(messages: &[Message]) -> Vec<Message> {
    messages.iter().map(|m| {
        let cleaned = sanitize_text(&m.content);
        Message {
            content: cleaned,
            ..m.clone()
        }
    }).collect()
}

/// Remove non-ASCII and surrogate characters
fn sanitize_text(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_ascii() || *c as u32 > 0xFFFF)
        .collect()
}

/// Check if a message sequence would exceed context limits
pub fn would_exceed_context(messages: &[LLMMessage], max_chars: usize) -> bool {
    let total: usize = messages.iter().map(|m| m.content.len() + 50).sum();
    total > max_chars
}

/// Estimate if tools would fit in context
pub fn tools_fit_in_context(defs: &[ToolDefinition], available_chars: usize) -> bool {
    let def_size: usize = defs.iter()
        .map(|d| d.name.len() + d.description.len() + 100)
        .sum();
    def_size < available_chars
}

/// Strip images from messages (for text-only models)
pub fn strip_images_from_messages(messages: &[Message]) -> Vec<Message> {
    // In our current impl, images aren't supported yet, so this is a no-op
    messages.to_vec()
}

/// Build conversation for resubmission (remove last N messages)
pub fn truncate_for_retry(conv: &mut Conversation, max_messages: usize) {
    if conv.messages.len() > max_messages {
        // Keep system message, remove some middle history
        let system_count = conv.messages.iter()
            .filter(|m| matches!(m.role, Role::System))
            .count();

        if conv.messages.len() > max_messages + system_count {
            let to_remove = conv.messages.len() - max_messages - system_count;
            let start = system_count;
            let end = (start + to_remove).min(conv.messages.len());
            conv.messages.drain(start..end);
        }
    }
}

/// Detect if the error is a context length exceeded error
pub fn is_context_length_error(error: &str) -> bool {
    let e = error.to_lowercase();
    e.contains("context_length") 
        || e.contains("max_tokens")
        || e.contains("token limit")
        || e.contains("maximum context")
        || e.contains("context window")
}

/// Detect auth/credential error  
pub fn is_auth_error(error: &str) -> bool {
    let e = error.to_lowercase();
    e.contains("401") 
        || e.contains("unauthorized")
        || e.contains("authentication")
        || e.contains("api key")
        || e.contains("invalid key")
        || e.contains("auth")
}

/// Format tool definitions into a compact text description
pub fn tool_defs_to_text(defs: &[ToolDefinition]) -> String {
    if defs.is_empty() {
        return "No tools available.".to_string();
    }

    let mut lines = Vec::new();
    for d in defs {
        let params = d.parameters.as_object()
            .and_then(|o| o.get("properties"))
            .and_then(|p| p.as_object())
            .map(|props| {
                props.keys().cloned().collect::<Vec<_>>().join(", ")
            })
            .unwrap_or_default();

        let params_str = if params.is_empty() { String::new() } else { format!("({})", params) };
        lines.push(format!("  - {}{}: {}", d.name, params_str, d.description));
    }

    format!("Available tools:\n{}", lines.join("\n"))
}
