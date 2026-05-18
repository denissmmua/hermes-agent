use std::sync::Arc;

use hermes_core::{
    AgentConfig, AgentError, AgentResult, AgentState, Conversation, Message, Role, ToolCall,
    ToolInput, ToolOutput,
};
use hermes_gateway::{LLMMessage, LLMProvider, LLMRequest, LLMResponse, OpenAIProvider};

pub struct AgentRuntime {
    state: AgentState,
    gateway: Arc<dyn LLMProvider>,
}

impl AgentRuntime {
    pub fn new(config: AgentConfig, gateway: Arc<dyn LLMProvider>) -> Self {
        let tools = Default::default();
        let state = AgentState::new(config, tools);
        Self { state, gateway }
    }

    pub fn state(&self) -> &AgentState {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut AgentState {
        &mut self.state
    }

    pub async fn run_turn(&mut self, user_input: &str) -> AgentResult<String> {
        // Add user message
        self.state.conversation.push(Message::user(user_input));
        self.state.turn += 1;

        // Check max turns
        if self.state.turn > self.state.config.max_turns {
            return Err(AgentError::Unknown("Max turns exceeded".into()));
        }

        // Get LLM response
        let llm_messages = to_llm_messages(&self.state.conversation);
        let request = LLMRequest {
            model: self.state.config.model.clone(),
            messages: llm_messages,
            max_tokens: Some(4096),
            temperature: Some(self.state.config.temperature),
            stop: None,
            stream: false,
        };

        let response = self.gateway.chat(request).await?;

        // Add assistant message
        self.state
            .conversation
            .push(Message::assistant(&response.content));

        Ok(response.content)
    }

    pub async fn run_tool_call(
        &mut self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> AgentResult<ToolOutput> {
        let tool = self
            .state
            .tools
            .get(tool_name)
            .ok_or_else(|| AgentError::Tool(format!("Tool '{}' not found", tool_name)))?;

        let input = ToolInput {
            tool_name: tool_name.to_string(),
            arguments,
        };

        let output = tool.execute(input).await?;
        Ok(output)
    }
}

fn to_llm_messages(conv: &Conversation) -> Vec<LLMMessage> {
    conv.messages
        .iter()
        .map(|m| {
            let role = match m.role {
                Role::System => "system",
                Role::User => "user",
                Role::Assistant => "assistant",
                Role::Tool => "tool",
            };
            LLMMessage {
                role: role.to_string(),
                content: m.content.clone(),
            }
        })
        .collect()
}
