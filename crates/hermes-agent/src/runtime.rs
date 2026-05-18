use std::sync::Arc;
use hermes_core::{
    AgentConfig, AgentError, AgentResult, AgentState, Message, Role, ToolInput,
};
use hermes_gateway::{LLMMessage, LLMProvider, LLMRequest, RetryConfig, with_retry};

pub struct AgentRuntime {
    state: AgentState,
    gateway: Arc<dyn LLMProvider>,
    retry_config: RetryConfig,
}

impl AgentRuntime {
    pub fn new(config: AgentConfig, gateway: Arc<dyn LLMProvider>) -> Self {
        let tools = Default::default();
        let state = AgentState::new(config, tools);
        Self {
            state,
            gateway,
            retry_config: RetryConfig::default(),
        }
    }

    pub fn state(&self) -> &AgentState {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut AgentState {
        &mut self.state
    }

    pub fn with_retry_config(mut self, config: RetryConfig) -> Self {
        self.retry_config = config;
        self
    }

    pub fn register_tool(&mut self, tool: Arc<dyn hermes_core::Tool>) {
        self.state.tools.register(tool);
    }

    pub async fn run_turn(&mut self, user_input: &str) -> AgentResult<String> {
        self.state.conversation.push(Message::user(user_input));
        self.state.turn += 1;

        if self.state.turn > self.state.config.max_turns {
            return Err(AgentError::Unknown("Max turns exceeded".into()));
        }

        let llm_messages = self.to_llm_messages();
        let model = self.state.config.model.clone();
        let temperature = self.state.config.temperature;

        let response = {
            let gw = &self.gateway;
            let model = model.clone();
            let msgs = llm_messages.clone();
            let temp = temperature;
            with_retry(&self.retry_config, || async {
                let request = LLMRequest {
                    model: model.clone(),
                    messages: msgs.clone(),
                    max_tokens: Some(4096),
                    temperature: Some(temp),
                    stop: None,
                    stream: false,
                };
                gw.chat(request).await
            })
            .await?
        };

        self.state
            .conversation
            .push(Message::assistant(&response.content));

        // Check for tool calls using XML-style tags
        let tool_calls = self.extract_tool_calls(&response.content);
        for (tool_name, args) in tool_calls {
            let result = match self.state.tools.get(&tool_name) {
                Some(tool) => {
                    let input = ToolInput {
                        tool_name,
                        arguments: args,
                    };
                    match tool.execute(input).await {
                        Ok(output) => format!("Result: {}", output.output),
                        Err(e) => format!("Error: {}", e),
                    }
                }
                None => format!("Tool '{}' not found", tool_name),
            };
            self.state
                .conversation
                .push(Message::assistant(result));
        }

        Ok(response.content)
    }

    fn to_llm_messages(&self) -> Vec<LLMMessage> {
        self.state
            .conversation
            .messages
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

    fn extract_tool_calls(&self, content: &str) -> Vec<(String, serde_json::Value)> {
        let mut calls = Vec::new();
        let chars: Vec<char> = content.chars().collect();
        let len = chars.len();
        let mut pos = 0;

        while pos < len {
            // Find opening <
            while pos < len && chars[pos] != '<' {
                pos += 1;
            }
            if pos >= len {
                break;
            }
            let _tag_start = pos;
            pos += 1; // skip <

            // Find tag name (until >)
            let mut tag_end = pos;
            while tag_end < len && chars[tag_end] != '>' {
                tag_end += 1;
            }
            if tag_end >= len {
                break;
            }
            let tag_name: String = chars[pos..tag_end].iter().collect();
            pos = tag_end + 1; // skip >

            // Find matching closing tag
            let close_tag: Vec<char> = format!("</{}>", tag_name).chars().collect();
            let close_len = close_tag.len();
            let mut close_pos = pos;

            // Check if there's a self-closing tag
            if tag_end > 0 && chars[tag_end - 1] == '/' {
                calls.push((tag_name, serde_json::Value::Null));
                continue;
            }

            while close_pos + close_len <= len {
                if &chars[close_pos..close_pos + close_len] == close_tag.as_slice() {
                    let inner: String = chars[pos..close_pos].iter().collect();
                    let args: serde_json::Value =
                        serde_json::from_str(inner.trim()).unwrap_or(serde_json::Value::String(inner.trim().to_string()));
                    calls.push((tag_name, args));
                    pos = close_pos + close_len;
                    break;
                }
                close_pos += 1;
            }

            if close_pos + close_len > len {
                break; // No matching close tag
            }
        }

        calls
    }
}
