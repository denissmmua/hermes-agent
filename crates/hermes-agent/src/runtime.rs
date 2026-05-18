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
        Self { state, gateway, retry_config: RetryConfig::default() }
    }

    pub fn state(&self) -> &AgentState { &self.state }
    pub fn state_mut(&mut self) -> &mut AgentState { &mut self.state }
    pub fn register_tool(&mut self, tool: Arc<dyn hermes_core::Tool>) {
        self.state.tools.register(tool);
    }

    /// One user turn: may involve multiple LLM calls if tool calls are made
    pub async fn run_turn(&mut self, user_input: &str) -> AgentResult<String> {
        self.state.conversation.push(Message::user(user_input));
        self.state.turn += 1;

        if self.state.turn > self.state.config.max_turns {
            return Err(hermes_core::AgentError::Unknown("Max turns exceeded".into()));
        }

        // Observe-Think-Act loop
        let mut final_response = String::new();
        let mut tool_calls_made = 0;

        loop {
            if tool_calls_made > 10 {
                final_response.push_str("\n[Max tool calls reached]");
                break;
            }

            let llm_messages = self.to_llm_messages();
            let tools = self.state.tools.all_definitions();
            let model = self.state.config.model.clone();
            let temperature = self.state.config.temperature;

            // 1. THINK: Ask LLM
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

            // Check if there are tool calls
            if response.tool_calls.is_empty() {
                // 3. ACT: No tool calls — final response
                if !response.content.is_empty() {
                    self.state.conversation.push(Message::assistant(&response.content));
                    final_response = response.content;
                }
                break;
            }

            // 2. ACT: Execute tool calls
            for (_call_id, tool_name, args) in &response.tool_calls {
                tool_calls_made += 1;
                let result = match self.state.tools.get(tool_name) {
                    Some(tool) => {
                        let input = ToolInput {
                            tool_name: tool_name.clone(),
                            arguments: args.clone(),
                        };
                        match tool.execute(input).await {
                            Ok(output) => {
                                let result = format!("Result: {}", output.output);
                                self.state.conversation.push(Message::assistant(&result));
                                result
                            }
                            Err(e) => {
                                let err = format!("Error: {}", e);
                                self.state.conversation.push(Message::assistant(&err));
                                err
                            }
                        }
                    }
                    None => {
                        let err = format!("Tool '{}' not found", tool_name);
                        self.state.conversation.push(Message::assistant(&err));
                        err
                    }
                };

                // Feed result back to LLM for next iteration
                let tool_result_msg = LLMMessage {
                    role: "tool".to_string(),
                    content: result.clone(),
                };
                // We'll include it in the next loop iteration
                final_response = result;
            }

            // Loop continues: THINK again with tool results in context
        }

        Ok(final_response)
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
