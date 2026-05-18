use async_trait::async_trait;
use hermes_core::{AgentError, AgentResult, GatewayConfig};
use reqwest::Client;

use crate::provider::{LLMMessage, LLMProvider, LLMRequest, LLMResponse, TokenUsage};

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

        let body = serde_json::json!({
            "model": request.model,
            "messages": request.messages.iter().map(|m| {
                serde_json::json!({"role": m.role, "content": m.content})
            }).collect::<Vec<serde_json::Value>>(),
            "max_tokens": request.max_tokens,
            "temperature": request.temperature,
            "stream": false,
        });

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
            return Err(AgentError::Provider(format!(
                "API error {}: {}",
                status, text
            )));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| AgentError::Provider(format!("Parse error: {}", e)))?;

        let content = json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();

        let usage = json["usage"].as_object().map(|u| TokenUsage {
            prompt_tokens: u["prompt_tokens"].as_u64().unwrap_or(0) as u32,
            completion_tokens: u["completion_tokens"].as_u64().unwrap_or(0) as u32,
            total_tokens: u["total_tokens"].as_u64().unwrap_or(0) as u32,
        });

        Ok(LLMResponse {
            content,
            model: request.model,
            usage,
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
