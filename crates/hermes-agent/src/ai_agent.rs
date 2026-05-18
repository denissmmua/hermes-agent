use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_yaml;
use hermes_core::{
    AgentConfig, AgentError, AgentResult, Conversation, GatewayConfig, Message,
    Role, Tool, ToolCall, ToolRegistry, ToolInput,
};
use hermes_gateway::{LLMMessage, LLMProvider, LLMRequest, LLMResponse, RetryConfig, with_retry};

use crate::compressor::ContextCompressor;
use crate::context::ContextEngine;

// ═══════════════════════════════════════════════════════════════════════
// Callback types
// ═══════════════════════════════════════════════════════════════════════

pub type ToolProgressCallback = Arc<dyn Fn(&str, &str) + Send + Sync>;
pub type StreamDeltaCallback = Arc<dyn Fn(&str) + Send + Sync>;
pub type ReasoningCallback = Arc<dyn Fn(&str) + Send + Sync>;
pub type StatusCallback = Arc<dyn Fn(&str) + Send + Sync>;

// ═══════════════════════════════════════════════════════════════════════
// Provider route — holds configuration for one LLM provider
// ═══════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct ProviderRoute {
    pub name: String,
    pub gateway: GatewayConfig,
    pub model: String,
    pub priority: u32,
}

// ═══════════════════════════════════════════════════════════════════════
// Iteration Budget
// ═══════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct IterationBudget {
    pub max_iterations: u32,
    pub used: u32,
}

impl IterationBudget {
    pub fn new(max: u32) -> Self {
        Self { max_iterations: max, used: 0 }
    }

    pub fn consume(&mut self) -> bool {
        if self.used >= self.max_iterations {
            false
        } else {
            self.used += 1;
            true
        }
    }

    pub fn remaining(&self) -> u32 {
        self.max_iterations.saturating_sub(self.used)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Credential Pool
// ═══════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct CredentialSource {
    pub provider: String,    // "openai", "deepseek", "openrouter", etc.
    pub api_key: String,
    pub base_url: String,
    pub model: String,
    pub priority: u32,
}

#[derive(Debug, Clone, Default)]
pub struct CredentialPool {
    pub sources: Vec<CredentialSource>,
    pub active_index: usize,
}

impl CredentialPool {
    pub fn new() -> Self { Self::default() }

    pub fn add(&mut self, source: CredentialSource) {
        self.sources.push(source);
        self.sources.sort_by_key(|s| s.priority);
    }

    pub fn active(&self) -> Option<&CredentialSource> {
        self.sources.get(self.active_index)
    }

    pub fn rotate(&mut self) -> Option<&CredentialSource> {
        if self.sources.is_empty() {
            return None;
        }
        self.active_index = (self.active_index + 1) % self.sources.len();
        self.sources.get(self.active_index)
    }

    /// Load from env vars and config
    pub fn load_default() -> Self {
        let mut pool = Self::new();

        // DeepSeek
        if let Ok(key) = std::env::var("DEEPSEEK_API_KEY").or_else(|_| std::env::var("OPENAI_API_KEY")) {
            if !key.is_empty() {
                let base = std::env::var("DEEPSEEK_BASE_URL")
                    .or_else(|_| std::env::var("OPENAI_BASE_URL"))
                    .unwrap_or_else(|_| "https://api.deepseek.com".to_string());
                let model = std::env::var("HERMES_MODEL").unwrap_or_else(|_| "deepseek-chat".to_string());
                pool.add(CredentialSource {
                    provider: "deepseek".to_string(),
                    api_key: key,
                    base_url: base,
                    model,
                    priority: 0,
                });
            }
        }

        // Try config file
        let config_path = shellexpand::tilde("~/.hermes/config.yaml");
        if let Ok(c) = std::fs::read_to_string(config_path.as_ref()) {
            if let Ok(cfg) = serde_yaml::from_str::<hermes_core::HermesConfig>(&c) {
                for cp in &cfg.custom_providers {
                    if !cp.api_key.is_empty() {
                        pool.add(CredentialSource {
                            provider: cp.name.clone(),
                            api_key: cp.api_key.clone(),
                            base_url: cp.base_url.clone(),
                            model: cp.model.clone(),
                            priority: pool.sources.len() as u32,
                        });
                    }
                }
            }
        }

        pool
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Session Metadata
// ═══════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub id: String,
    pub platform: String,
    pub user_id: String,
    pub user_name: String,
    pub chat_id: String,
    pub chat_name: String,
    pub chat_type: String,
    pub thread_id: String,
    pub created_at: std::time::Instant,
}

impl Default for SessionInfo {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            platform: "cli".to_string(),
            user_id: String::new(),
            user_name: String::new(),
            chat_id: String::new(),
            chat_name: String::new(),
            chat_type: String::new(),
            thread_id: String::new(),
            created_at: std::time::Instant::now(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Provider Registry — manages multiple providers with fallback
// ═══════════════════════════════════════════════════════════════════════

pub struct ProviderRegistry {
    /// Primary providers by name
    providers: HashMap<String, Arc<dyn LLMProvider>>,
    /// Ordered list of fallback provider names
    pub fallback_order: Vec<String>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
            fallback_order: Vec::new(),
        }
    }

    pub fn register(&mut self, name: &str, provider: Arc<dyn LLMProvider>) {
        self.providers.insert(name.to_string(), provider);
        if !self.fallback_order.contains(&name.to_string()) {
            self.fallback_order.push(name.to_string());
        }
    }

    pub fn get(&self, name: &str) -> Option<&Arc<dyn LLMProvider>> {
        self.providers.get(name)
    }

    pub fn primary(&self) -> Option<&Arc<dyn LLMProvider>> {
        self.fallback_order.first().and_then(|n| self.providers.get(n))
    }

    pub fn fallback_chain(&self, start_from: usize) -> Vec<&Arc<dyn LLMProvider>> {
        self.fallback_order.iter()
            .skip(start_from)
            .filter_map(|n| self.providers.get(n))
            .collect()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tool Call Guardrails
// ═══════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct GuardrailConfig {
    pub warnings_enabled: bool,
    pub hard_stop_enabled: bool,
    pub warn_after_exact_failure: u32,
    pub warn_after_same_tool_failure: u32,
    pub warn_after_idempotent_no_progress: u32,
    pub hard_stop_after_exact_failure: u32,
    pub hard_stop_after_same_tool_failure: u32,
    pub hard_stop_after_idempotent_no_progress: u32,
}

impl Default for GuardrailConfig {
    fn default() -> Self {
        Self {
            warnings_enabled: true,
            hard_stop_enabled: false,
            warn_after_exact_failure: 2,
            warn_after_same_tool_failure: 3,
            warn_after_idempotent_no_progress: 2,
            hard_stop_after_exact_failure: 5,
            hard_stop_after_same_tool_failure: 8,
            hard_stop_after_idempotent_no_progress: 5,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct GuardrailState {
    pub consecutive_failures: u32,
    pub same_tool_failures: HashMap<String, u32>,
    pub idempotent_no_progress_count: u32,
    pub last_tool_name: String,
    pub last_tool_result: String,
}

impl GuardrailState {
    pub fn record_failure(&mut self, tool_name: &str) {
        self.consecutive_failures += 1;
        *self.same_tool_failures.entry(tool_name.to_string()).or_insert(0) += 1;
    }

    pub fn record_success(&mut self) {
        self.consecutive_failures = 0;
        // Don't reset same_tool_failures — that's cumulative
    }

    pub fn check(&self, config: &GuardrailConfig) -> GuardrailVerdict {
        if config.hard_stop_enabled {
            if self.consecutive_failures >= config.hard_stop_after_exact_failure {
                return GuardrailVerdict::HardStop("Too many consecutive failures".into());
            }
            for (tool, count) in &self.same_tool_failures {
                if *count >= config.hard_stop_after_same_tool_failure {
                    return GuardrailVerdict::HardStop(format!("Tool {} failed too many times", tool));
                }
            }
        }

        if config.warnings_enabled {
            if self.consecutive_failures >= config.warn_after_exact_failure {
                return GuardrailVerdict::Warning(format!("{} consecutive tool failures", self.consecutive_failures));
            }
            for (tool, count) in &self.same_tool_failures {
                if *count >= config.warn_after_same_tool_failure {
                    return GuardrailVerdict::Warning(format!("Tool {} failed {} times", tool, count));
                }
            }
        }

        GuardrailVerdict::Ok
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum GuardrailVerdict {
    Ok,
    Warning(String),
    HardStop(String),
}

// ═══════════════════════════════════════════════════════════════════════
// The Main AIAgent
// ═══════════════════════════════════════════════════════════════════════

pub struct AIAgent {
    // Configuration
    pub model: String,
    pub max_iterations: u32,
    pub max_tokens: Option<u32>,
    pub temperature: f64,
    pub tool_delay: Duration,
    pub verbose: bool,

    // Runtime state
    pub conversation: Conversation,
    pub tools: ToolRegistry,
    pub session: SessionInfo,
    pub turn: u32,

    // Credentials & providers
    pub credential_pool: CredentialPool,
    pub provider_registry: ProviderRegistry,
    pub active_provider_name: String,
    pub fallback_models: Vec<HashMap<String, String>>,

    // Budget
    pub iteration_budget: IterationBudget,

    // Guardrails
    pub guardrail_config: GuardrailConfig,
    pub guardrail_state: GuardrailState,

    // Context management
    pub compressor: Option<ContextCompressor>,
    pub context_engine: Option<ContextEngine>,

    // Retry
    pub retry_config: RetryConfig,
}

impl AIAgent {
    /// Create a new AIAgent with defaults from env/config
    pub fn new() -> Self {
        let pool = CredentialPool::load_default();
        let active = pool.active().cloned().unwrap_or(CredentialSource {
            provider: "deepseek".to_string(),
            api_key: String::new(),
            base_url: "https://api.deepseek.com".to_string(),
            model: "deepseek-chat".to_string(),
            priority: 0,
        });

        let mut registry = ProviderRegistry::new();
        if !active.api_key.is_empty() {
            let gw = GatewayConfig {
                provider: active.provider.clone(),
                model: active.model.clone(),
                api_key: active.api_key.clone(),
                base_url: Some(active.base_url.clone()),
                ..Default::default()
            };
            registry.register(&active.provider, Arc::new(hermes_gateway::OpenAIProvider::new(gw)));
        }

        let model_name = active.model.clone();
        let provider_name = active.provider.clone();

        let mut tools = ToolRegistry::new();

        Self {
            model: model_name,
            max_iterations: 90,
            max_tokens: None,
            temperature: 0.7,
            tool_delay: Duration::from_millis(500),
            verbose: false,
            conversation: Conversation::new(),
            tools,
            session: SessionInfo::default(),
            turn: 0,
            credential_pool: pool,
            provider_registry: registry,
            active_provider_name: provider_name,
            fallback_models: Vec::new(),
            iteration_budget: IterationBudget::new(90),
            guardrail_config: GuardrailConfig::default(),
            guardrail_state: GuardrailState::default(),
            compressor: None,
            context_engine: None,
            retry_config: RetryConfig::default(),
        }
    }

    /// Register a tool
    pub fn register_tool(&mut self, tool: Arc<dyn Tool>) {
        self.tools.register(tool);
    }

    /// Register multiple tools
    pub fn register_tools(&mut self, tools: Vec<Arc<dyn Tool>>) {
        for t in tools {
            self.tools.register(t);
        }
    }

    /// Set system prompt
    pub fn set_system_prompt(&mut self, prompt: &str) {
        // Insert or replace system message at position 0
        if let Some(first) = self.conversation.messages.first_mut() {
            if matches!(first.role, Role::System) {
                first.content = prompt.to_string();
                return;
            }
        }
        self.conversation.messages.insert(0, Message::system(prompt));
    }

    /// Get the active LLM provider
    fn get_provider(&self) -> AgentResult<&Arc<dyn LLMProvider>> {
        self.provider_registry.get(&self.active_provider_name)
            .or_else(|| self.provider_registry.primary())
            .ok_or_else(|| AgentError::Config("No LLM provider configured".into()))
    }

    /// Convert conversation messages to LLM format
    fn to_llm_messages(&self) -> Vec<LLMMessage> {
        self.conversation.messages.iter().map(|m| {
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

    /// Full observe-think-act loop
    pub async fn run_turn(&mut self, user_input: &str) -> AgentResult<String> {
        self.conversation.push(Message::user(user_input));
        self.turn += 1;

        if self.turn > self.max_iterations {
            return Err(AgentError::Unknown("Max turns exceeded".into()));
        }

        let provider = self.get_provider()?.clone();
        let tool_defs = self.tools.all_definitions();
        let model = self.model.clone();
        let temperature = self.temperature;
        let max_tokens = self.max_tokens;
        let guard_config = self.guardrail_config.clone();
        let max_loop = 15u32;

        for _iteration in 0..max_loop {
            // Check guardrails
            {
                let verdict = self.guardrail_state.check(&guard_config);
                match verdict {
                    GuardrailVerdict::HardStop(msg) => {
                        self.conversation.push(Message::assistant(&format!("[Stopped: {}]", msg)));
                        return Err(AgentError::Unknown(msg));
                    }
                    GuardrailVerdict::Warning(msg) => {
                        if self.verbose {
                            eprintln!("⚠ Guardrail: {}", msg);
                        }
                    }
                    GuardrailVerdict::Ok => {}
                }
            }

            // Check iteration budget
            if !self.iteration_budget.consume() {
                self.conversation.push(Message::assistant("[Iteration budget exhausted]"));
                return Err(AgentError::Unknown("Iteration budget exhausted".into()));
            }

            // Build request — extract messages before mutable borrow
            let llm_messages = self.to_llm_messages();
            let request = LLMRequest {
                model: model.clone(),
                messages: llm_messages,
                max_tokens,
                temperature: Some(temperature),
                stop: None,
                stream: false,
                tools: tool_defs.clone(),
            };

            let response = with_retry(&self.retry_config, || async {
                provider.chat(request.clone()).await
            }).await?;

            // Store assistant response
            {
                let mut assistant_msg = Message::assistant(&response.content);
                if !response.tool_calls.is_empty() {
                    let calls: Vec<ToolCall> = response.tool_calls.iter().map(|(id, name, args)| {
                        ToolCall {
                            id: id.clone(),
                            name: name.clone(),
                            arguments: args.clone(),
                        }
                    }).collect();
                    assistant_msg = assistant_msg.with_tool_calls(calls);
                }
                self.conversation.push(assistant_msg);
            }

            if response.tool_calls.is_empty() {
                return Ok(response.content);
            }

            // Execute tool calls
            for (call_id, tool_name, args) in &response.tool_calls {
                let start = Instant::now();
                let (output, success) = match self.tools.get(tool_name) {
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

                // Update guardrails
                if success {
                    self.guardrail_state.record_success();
                } else {
                    self.guardrail_state.record_failure(tool_name);
                }

                self.conversation.push(
                    Message::tool_result_msg(call_id, tool_name, &output, success, duration_ms)
                );

                // Tool delay
                if !self.tool_delay.is_zero() {
                    tokio::time::sleep(self.tool_delay).await;
                }
            }
        }

        let last = self.conversation.messages.iter()
            .filter(|m| matches!(m.role, Role::Assistant))
            .last()
            .map(|m| m.content.clone())
            .unwrap_or_default();
        Ok(last)
    }

    /// Reset conversation but keep config
    pub fn reset(&mut self) {
        self.conversation = Conversation::new();
        self.turn = 0;
        self.guardrail_state = GuardrailState::default();
        self.iteration_budget = IterationBudget::new(self.max_iterations);
    }

    /// Get conversation stats
    pub fn stats(&self) -> AgentStats {
        AgentStats {
            model: self.model.clone(),
            turn: self.turn,
            messages: self.conversation.messages.len() as u32,
            context_length: self.conversation.context_length(),
            budget_remaining: self.iteration_budget.remaining(),
            provider: self.active_provider_name.clone(),
        }
    }
}

impl Default for AIAgent {
    fn default() -> Self { Self::new() }
}

#[derive(Debug, Clone)]
pub struct AgentStats {
    pub model: String,
    pub turn: u32,
    pub messages: u32,
    pub context_length: usize,
    pub budget_remaining: u32,
    pub provider: String,
}

impl std::fmt::Display for AgentStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Model: {} | Turn: {} | Msgs: {} | Context: {} | Budget: {} | Provider: {}",
            self.model,
            self.turn,
            self.messages,
            self.context_length,
            self.budget_remaining,
            self.provider,
        )
    }
}
