use crate::message::{Conversation, Message};
use crate::tool::ToolRegistry;
use crate::error::AgentResult;

#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub name: String,
    pub system_prompt: String,
    pub model: String,
    pub max_turns: u32,
    pub max_tokens: usize,
    pub temperature: f64,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            name: "hermes".to_string(),
            system_prompt: "You are Hermes, a helpful AI agent.".to_string(),
            model: "deepseek-v4-flash".to_string(),
            max_turns: 50,
            max_tokens: 128_000,
            temperature: 0.7,
        }
    }
}

pub struct AgentState {
    pub conversation: Conversation,
    pub turn: u32,
    pub config: AgentConfig,
    pub tools: ToolRegistry,
}

impl AgentState {
    pub fn new(config: AgentConfig, tools: ToolRegistry) -> Self {
        let mut conv = Conversation::new();
        conv.push(Message::system(config.system_prompt.clone()));
        Self {
            conversation: conv,
            turn: 0,
            config,
            tools,
        }
    }
}
