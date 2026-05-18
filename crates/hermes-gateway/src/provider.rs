use async_trait::async_trait;
use hermes_core::{AgentResult, GatewayConfig, ToolDefinition};

#[derive(Debug, Clone)]
pub struct LLMRequest {
    pub model: String,
    pub messages: Vec<LLMMessage>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f64>,
    pub stop: Option<Vec<String>>,
    pub stream: bool,
    pub tools: Vec<ToolDefinition>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LLMMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<LLMToolCall>>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LLMToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub function: LLMToolCallFunction,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LLMToolCallFunction {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone)]
pub struct LLMResponse {
    pub content: String,
    pub model: String,
    pub usage: Option<TokenUsage>,
    pub tool_calls: Vec<(String, String, serde_json::Value)>, // id, name, arguments
}

#[derive(Debug, Clone)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[async_trait]
pub trait LLMProvider: Send + Sync {
    fn name(&self) -> &str;
    async fn chat(&self, request: LLMRequest) -> AgentResult<LLMResponse>;
    async fn chat_stream(
        &self,
        request: LLMRequest,
        callback: Box<dyn FnMut(String) + Send>,
    ) -> AgentResult<LLMResponse>;
}
