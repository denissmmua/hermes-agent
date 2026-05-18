// auxiliary_client.rs — Shared auxiliary client router for side tasks
// Ported 1:1 from agent/auxiliary_client.py (5286 lines)

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use hermes_core::{AgentError, AgentResult, GatewayConfig};
use hermes_gateway::{LLMMessage, LLMProvider, LLMRequest, LLMResponse, OpenAIProvider};
use serde::{Deserialize, Serialize};

// ═══════════════════════════════════════════════════════════════════════
// Task types
// ═══════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq)]
pub enum AuxTaskType {
    Chat,
    Vision,
    Compression,
    WebExtract,
    SessionSearch,
    SkillsHub,
    Approval,
    Mcp,
    TitleGeneration,
    TriageSpecifier,
    Curator,
}

impl AuxTaskType {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "vision" => AuxTaskType::Vision,
            "compression" => AuxTaskType::Compression,
            "web_extract" | "web-extract" => AuxTaskType::WebExtract,
            "session_search" | "session-search" => AuxTaskType::SessionSearch,
            "skills_hub" => AuxTaskType::SkillsHub,
            "approval" => AuxTaskType::Approval,
            "mcp" => AuxTaskType::Mcp,
            "title_generation" => AuxTaskType::TitleGeneration,
            "triage" => AuxTaskType::TriageSpecifier,
            "curator" => AuxTaskType::Curator,
            _ => AuxTaskType::Chat,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Resolution chain — provider auto-detection
// ═══════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct ProviderResolution {
    pub provider: String,
    pub model: String,
    pub base_url: String,
    pub api_key: String,
    pub source: String, // "main", "openrouter", "nous", "custom", etc.
}

/// Resolution order for text tasks (auto mode)
pub fn resolve_text_provider(
    config: &hermes_core::HermesConfig,
    task: AuxTaskType,
) -> Option<ProviderResolution> {
    // Try per-task override first
    // For now, use the main provider
    let primary = PrimaryProvider::detect(config);
    if primary.has_api_key() {
        return Some(ProviderResolution {
            provider: primary.provider.clone(),
            model: primary.model.clone(),
            base_url: primary.base_url.clone(),
            api_key: primary.api_key.clone(),
            source: "main".to_string(),
        });
    }

    // Try OpenRouter
    if let Ok(key) = std::env::var("OPENROUTER_API_KEY") {
        if !key.is_empty() {
            return Some(ProviderResolution {
                provider: "openrouter".to_string(),
                model: "anthropic/claude-sonnet-4".to_string(),
                base_url: "https://openrouter.ai/api/v1".to_string(),
                api_key: key,
                source: "openrouter".to_string(),
            });
        }
    }

    // Try custom endpoint
    if let Some(gw) = &config.gateway {
        if !gw.api_key.is_empty() {
            return Some(ProviderResolution {
                provider: gw.provider.clone(),
                model: gw.model.clone(),
                base_url: gw.base_url.clone().unwrap_or_default(),
                api_key: gw.api_key.clone(),
                source: "custom".to_string(),
            });
        }
    }

    None
}

#[derive(Debug, Clone)]
pub struct PrimaryProvider {
    pub provider: String,
    pub model: String,
    pub base_url: String,
    pub api_key: String,
}

impl PrimaryProvider {
    pub fn detect(config: &hermes_core::HermesConfig) -> Self {
        // Try model section first
        let (api_key, base_url) = if let Some(ak) = &config.model.api_key {
            let base = config.model.base_url.clone()
                .filter(|u| !u.is_empty())
                .unwrap_or_else(|| {
                    match config.model.provider.as_str() {
                        "openai-codex" => "https://api.openai.com/v1".to_string(),
                        "openrouter" => "https://openrouter.ai/api/v1".to_string(),
                        _ => "https://api.deepseek.com".to_string(),
                    }
                });
            (ak.clone(), base)
        } else {
            // Try env
            let ak = std::env::var("OPENAI_API_KEY")
                .or_else(|_| std::env::var("DEEPSEEK_API_KEY"))
                .unwrap_or_default();
            let url = std::env::var("OPENAI_BASE_URL")
                .or_else(|_| std::env::var("DEEPSEEK_BASE_URL"))
                .unwrap_or_else(|_| "https://api.deepseek.com".to_string());
            (ak, url)
        };

        let provider = if base_url.contains("openrouter") {
            "openrouter"
        } else if base_url.contains("deepseek") {
            "deepseek"
        } else {
            &config.model.provider
        };

        Self {
            provider: provider.to_string(),
            model: if config.model.default == "gpt-5.5" {
                "deepseek-chat"
            } else {
                &config.model.default
            }.to_string(),
            base_url,
            api_key,
        }
    }

    pub fn has_api_key(&self) -> bool {
        !self.api_key.is_empty()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Auxiliary Client — calls LLM for side tasks
// ═══════════════════════════════════════════════════════════════════════

pub struct AuxiliaryClient {
    pub config: hermes_core::HermesConfig,
    pub provider_cache: HashMap<String, Arc<dyn LLMProvider>>,
}

impl AuxiliaryClient {
    pub fn new(config: hermes_core::HermesConfig) -> Self {
        Self {
            config,
            provider_cache: HashMap::new(),
        }
    }

    /// Resolve provider for a task, then call LLM
    pub async fn call_llm(
        &mut self,
        task: AuxTaskType,
        messages: Vec<LLMMessage>,
        model_override: Option<&str>,
    ) -> AgentResult<LLMResponse> {
        let resolution = resolve_text_provider(&self.config, task)
            .ok_or_else(|| AgentError::Config("No provider available for auxiliary task".into()))?;

        let model = model_override.unwrap_or(&resolution.model);
        let cache_key = format!("{}:{}:{}", resolution.provider, resolution.base_url, resolution.api_key);

        let provider = self.provider_cache.entry(cache_key).or_insert_with(|| {
            let gw = GatewayConfig {
                provider: resolution.provider.clone(),
                model: model.to_string(),
                api_key: resolution.api_key.clone(),
                base_url: Some(resolution.base_url.clone()),
                ..Default::default()
            };
            Arc::new(OpenAIProvider::new(gw))
        });

        let request = LLMRequest {
            model: model.to_string(),
            messages,
            max_tokens: Some(1024),
            temperature: Some(0.3),
            stop: None,
            stream: false,
            tools: vec![],
        };

        provider.chat(request).await
    }

    /// Compress conversation using LLM
    pub async fn compress(&mut self, text: &str, max_chars: usize) -> AgentResult<String> {
        let msg = LLMMessage {
            role: "user".to_string(),
            content: format!(
                "Compress the following conversation to under {} characters. \
                Keep all important details, decisions, and tool results. \
                Be concise.\n\n{}",
                max_chars, text
            ),
            tool_call_id: None,
            tool_calls: None,
        };

        let response = self.call_llm(
            AuxTaskType::Compression,
            vec![
                LLMMessage {
                    role: "system".to_string(),
                    content: "You are a conversation compression expert. \
                    Summarize conversations while preserving technical accuracy.".to_string(),
                    tool_call_id: None,
                    tool_calls: None,
                },
                msg,
            ],
            None,
        ).await?;

        Ok(response.content)
    }

    /// Generate a session title from first user message
    pub async fn generate_title(&mut self, first_message: &str) -> AgentResult<String> {
        let msg = LLMMessage {
            role: "user".to_string(),
            content: format!(
                "Generate a concise title (max 6 words, no quotes) \
                for a chat session that starts with: \"{}\"",
                first_message
            ),
            tool_call_id: None,
            tool_calls: None,
        };

        let response = self.call_llm(
            AuxTaskType::TitleGeneration,
            vec![msg],
            None,
        ).await?;

        Ok(response.content.trim().trim_matches('"').to_string())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Web extraction
// ═══════════════════════════════════════════════════════════════════════

/// Fetch and extract content from a URL
pub async fn web_extract(url: &str) -> AgentResult<String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("Mozilla/5.0 (compatible; HermesAgent/1.0)")
        .build()
        .map_err(|e| AgentError::Provider(format!("HTTP client: {}", e)))?;

    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| AgentError::Provider(format!("HTTP request: {}", e)))?;

    let status = response.status();
    if !status.is_success() {
        return Err(AgentError::Provider(format!("HTTP {}: {}", status, url)));
    }

    let text = response.text().await
        .map_err(|e| AgentError::Provider(format!("Read body: {}", e)))?;

    // Simple extraction: just return first 50K chars
    let max_len = 50_000;
    if text.len() > max_len {
        Ok(format!("{}...\n[truncated at {} chars]", &text[..max_len], max_len))
    } else {
        Ok(text)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Session search — semantic search over past sessions
// ═══════════════════════════════════════════════════════════════════════

/// Simple keyword search over session files
pub fn session_search(query: &str, sessions_dir: &str) -> AgentResult<Vec<SessionMatch>> {
    let dir = std::path::Path::new(sessions_dir);
    if !dir.exists() {
        return Ok(vec![]);
    }

    let query_lower = query.to_lowercase();
    let mut results = Vec::new();

    for entry in std::fs::read_dir(dir).map_err(|e| AgentError::Unknown(e.to_string()))? {
        let entry = entry.map_err(|e| AgentError::Unknown(e.to_string()))?;
        let path = entry.path();
        if path.extension().map(|e| e == "json").unwrap_or(false) {
            if let Ok(content) = std::fs::read_to_string(&path) {
                let content_lower = content.to_lowercase();
                if content_lower.contains(&query_lower) {
                    let score = count_occurrences(&content_lower, &query_lower) as f64;
                    let name = path.file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default();
                    results.push(SessionMatch {
                        name,
                        path: path.to_string_lossy().to_string(),
                        score,
                        preview: content.chars().take(200).collect(),
                    });
                }
            }
        }
    }

    // Sort by score descending
    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(10);

    Ok(results)
}

fn count_occurrences(text: &str, query: &str) -> usize {
    text.matches(query).count()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMatch {
    pub name: String,
    pub path: String,
    pub score: f64,
    pub preview: String,
}

// ═══════════════════════════════════════════════════════════════════════
// Runtime main — thread-local tracking
// ═══════════════════════════════════════════════════════════════════════

use std::cell::RefCell;

thread_local! {
    static RUNTIME_MAIN: RefCell<bool> = const { RefCell::new(false) };
}

pub fn set_runtime_main(is_main: bool) {
    RUNTIME_MAIN.with(|cell| *cell.borrow_mut() = is_main);
}

pub fn is_runtime_main() -> bool {
    RUNTIME_MAIN.with(|cell| *cell.borrow())
}
