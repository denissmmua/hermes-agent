use std::sync::Arc;
use std::time::Instant;
use hermes_core::{
    AgentConfig, AgentResult, AgentState, Message, Role, ToolDefinition, ToolInput,
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
        self.state_mut().tools.register(tool);
    }

    /// Full Hermes observe-think-act loop.
    /// 1. OBSERVE: user input → store as user message
    /// 2. THINK: send full conversation to LLM
    /// 3. ACT: if tool_calls → execute each, store results as tool messages, goto 2
    ///         if text → store as assistant response, done
    pub async fn run_turn(&mut self, user_input: &str) -> AgentResult<String> {
        // 1. OBSERVE
        self.state.conversation.push(Message::user(user_input));
        self.state.turn += 1;

        if self.state.turn > self.state.config.max_turns {
            return Err(hermes_core::AgentError::Unknown("Max turns exceeded".into()));
        }

        let max_iterations: usize = 15;
        let mut iteration: usize = 0;

        loop {
            iteration += 1;
            if iteration > max_iterations {
                self.state.conversation.push(Message::assistant("[Max iterations reached]"));
                break;
            }

            // 2. THINK: send conversation to LLM
            let llm_messages = self.to_llm_messages();
            let tool_defs = self.state.tools.all_definitions();
            let model = self.state.config.model.clone();
            let temperature = self.state.config.temperature;

            let response = {
                let gw = &self.gateway;
                let msgs = llm_messages.clone();
                let tdefs = tool_defs.clone();
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
                        tools: tdefs.clone(),
                    };
                    gw.chat(request).await
                })
                .await?
            };

            // Store assistant response (with tool_calls if any)
            let mut assistant_msg = Message::assistant(&response.content);
            if !response.tool_calls.is_empty() {
                let calls: Vec<hermes_core::ToolCall> = response.tool_calls.iter().map(|(id, name, args)| {
                    hermes_core::ToolCall {
                        id: id.clone(),
                        name: name.clone(),
                        arguments: args.clone(),
                    }
                }).collect();
                assistant_msg = assistant_msg.with_tool_calls(calls);
            }
            self.state.conversation.push(assistant_msg);

            // 3. ACT: execute tool calls if any
            if response.tool_calls.is_empty() {
                // No tool calls → this is the final response
                return Ok(response.content);
            }

            // Execute each tool call and store results
            for (call_id, tool_name, args) in &response.tool_calls {
                let start = Instant::now();
                let (output, success) = match self.state.tools.get(tool_name) {
                    Some(tool) => {
                        let input = ToolInput {
                            tool_name: tool_name.clone(),
                            arguments: args.clone(),
                        };
                        match tool.execute(input).await {
                            Ok(out) => (out.output, true),
                            Err(e) => (format!("Error: {}", e), false),
                        }
                    }
                    None => (format!("Tool '{}' not found", tool_name), false),
                };
                let duration_ms = start.elapsed().as_millis() as u64;

                // Store tool result as a tool-role message
                self.state.conversation.push(
                    Message::tool_result_msg(call_id, tool_name, &output, success, duration_ms)
                );
            }

            // Loop continues: tool results are now in conversation
            // LLM will see them on next iteration
        }

        // Get the last assistant message as final response
        let last = self.state.conversation.messages.iter()
            .filter(|m| matches!(m.role, Role::Assistant))
            .last()
            .map(|m| m.content.clone())
            .unwrap_or_default();

        Ok(last)
    }

    fn to_llm_messages(&self) -> Vec<LLMMessage> {
        self.state.conversation.messages.iter().map(|m| {
            let role = match m.role {
                Role::System => "system",
                Role::User => "user",
                Role::Assistant => "assistant",
                Role::Tool => "tool",
            };
            let tool_call_id = m.tool_result.as_ref().map(|r| r.call_id.clone());
            let tool_calls = m.tool_calls.as_ref().map(|calls| {
                calls.iter().map(|tc| {
                    let args_str = serde_json::to_string(&tc.arguments).unwrap_or_default();
                    hermes_gateway::LLMToolCall {
                        id: tc.id.clone(),
                        type_: "function".to_string(),
                        function: hermes_gateway::LLMToolCallFunction {
                            name: tc.name.clone(),
                            arguments: args_str,
                        },
                    }
                }).collect()
            });
            LLMMessage {
                role: role.to_string(),
                content: m.content.clone(),
                tool_call_id,
                tool_calls,
            }
        }).collect()
    }
}
