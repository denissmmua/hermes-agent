use async_trait::async_trait;
use hermes_core::{AgentError, AgentResult, GatewayConfig, ToolDefinition};
use reqwest::Client;

use crate::provider::{LLMProvider, LLMRequest, LLMResponse, TokenUsage};

#[derive(Clone)]
pub struct OpenAIProvider {
    client: Client,
    config: GatewayConfig,
}

impl OpenAIProvider {
    pub fn new(config: GatewayConfig) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .build()
            .unwrap_or_default();
        Self { client, config }
    }

    fn api_url(&self) -> String {
        self.config
            .base_url
            .clone()
            .unwrap_or_else(|| "https://api.openai.com/v1".to_string())
    }
}

#[async_trait]
impl LLMProvider for OpenAIProvider {
    fn name(&self) -> &str {
        &self.config.provider
    }

    async fn chat(&self, request: LLMRequest) -> AgentResult<LLMResponse> {
        let url = format!("{}/chat/completions", self.api_url());

        let mut body = serde_json::json!({
            "model": request.model,
            "messages": request.messages.iter().map(|m| {
                let mut msg = serde_json::json!({"role": m.role, "content": m.content});
                if let Some(tcid) = &m.tool_call_id {
                    msg["tool_call_id"] = serde_json::Value::String(tcid.clone());
                }
                if m.role == "assistant" {
                    if let Some(tcs) = &m.tool_calls {
                        let calls: Vec<serde_json::Value> = tcs.iter().map(|tc| {
                            serde_json::json!({
                                "id": tc.id,
                                "type": tc.type_,
                                "function": {
                                    "name": tc.function.name,
                                    "arguments": tc.function.arguments,
                                }
                            })
                        }).collect();
                        msg["tool_calls"] = serde_json::Value::Array(calls);
                    }
                }
                msg
            }).collect::<Vec<serde_json::Value>>(),
            "max_tokens": request.max_tokens,
            "temperature": request.temperature,
            "stream": false,
        });

        // Add native function calling tools
        let tools_json: Vec<serde_json::Value> = request.tools.iter().map(|t| {
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": t.name,
                    "description": t.description,
                    "parameters": t.parameters,
                }
            })
        }).collect();

        if !tools_json.is_empty() {
            body["tools"] = serde_json::Value::Array(tools_json);
            body["tool_choice"] = serde_json::Value::String("auto".to_string());
        }

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| AgentError::Provider(format!("HTTP error: {}", e)))?;

        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            if status.as_u16() == 429 {
                return Err(AgentError::RateLimited(10));
            }
            return Err(AgentError::Provider(format!("API error {}: {}", status, text)));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| AgentError::Provider(format!("Parse error: {}", e)))?;

        let message = &json["choices"][0]["message"];
        let content = message["content"].as_str().unwrap_or("").to_string();

        // Extract native function calls
        let mut tool_calls = Vec::new();
        if let Some(calls) = message["tool_calls"].as_array() {
            for tc in calls {
                let id = tc["id"].as_str().unwrap_or("").to_string();
                let name = tc["function"]["name"].as_str().unwrap_or("").to_string();
                let args_str = tc["function"]["arguments"].as_str().unwrap_or("{}");
                let args: serde_json::Value = serde_json::from_str(args_str).unwrap_or(serde_json::Value::Null);
                tool_calls.push((id, name, args));
            }
        }

        let usage = json["usage"].as_object().map(|u| TokenUsage {
            prompt_tokens: u["prompt_tokens"].as_u64().unwrap_or(0) as u32,
            completion_tokens: u["completion_tokens"].as_u64().unwrap_or(0) as u32,
            total_tokens: u["total_tokens"].as_u64().unwrap_or(0) as u32,
        });

        Ok(LLMResponse {
            content,
            model: request.model,
            usage,
            tool_calls,
        })
    }

    async fn chat_stream(
        &self,
        _request: LLMRequest,
        _callback: Box<dyn FnMut(String) + Send>,
    ) -> AgentResult<LLMResponse> {
        Err(AgentError::Provider("Streaming not yet implemented".into()))
    }
}
