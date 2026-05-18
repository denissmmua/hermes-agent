use std::sync::Arc;
use hermes_core::{
    AgentConfig, AgentResult, AgentState, Message, Role, ToolInput,
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

    pub fn state(&self) -> &AgentState { &self.state }
    pub fn state_mut(&mut self) -> &mut AgentState { &mut self.state }

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
            return Err(hermes_core::AgentError::Unknown("Max turns exceeded".into()));
        }

        // Build request with tools
        let llm_messages = self.to_llm_messages();
        let tools = self.state.tools.all_definitions();
        let model = self.state.config.model.clone();
        let temperature = self.state.config.temperature;

        let response = {
            let gw = &self.gateway;
            let msgs = llm_messages.clone();
            let tg = tools.clone();
            let mdl = model.clone();
            let temp = temperature;
            with_retry(&self.retry_config, || async {
                let request = LLMRequest {
                    model: mdl.clone(),
                    messages: msgs.clone(),
                    max_tokens: Some(4096),
                    temperature: Some(temp),
                    stop: None,
                    stream: false,
                    tools: tg.clone(),
                };
                gw.chat(request).await
            })
            .await?
        };

        // Handle tool calls from native function calling
        if !response.tool_calls.is_empty() {
            for (_call_id, tool_name, args) in &response.tool_calls {
                let result = match self.state.tools.get(tool_name) {
                    Some(tool) => {
                        let input = ToolInput {
                            tool_name: tool_name.clone(),
                            arguments: args.clone(),
                        };
                        match tool.execute(input).await {
                            Ok(output) => {
                                format!("Tool {} result: {}", tool_name, output.output)
                            }
                            Err(e) => format!("Tool {} error: {}", tool_name, e),
                        }
                    }
                    None => format!("Tool '{}' not found", tool_name),
                };
                self.state.conversation.push(
                    Message::assistant(result)
                );
            }
            // Return first content or tool call info
            return Ok(response.content);
        }

        self.state.conversation.push(Message::assistant(&response.content));
        Ok(response.content)
    }

    fn to_llm_messages(&self) -> Vec<LLMMessage> {
        self.state.conversation.messages.iter().map(|m| {
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
        }).collect()
    }
}
