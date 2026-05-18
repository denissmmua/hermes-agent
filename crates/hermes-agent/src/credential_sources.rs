use std::collections::HashMap;
use std::path::Path;

use hermes_core::{AgentResult, AgentError, GatewayConfig};
use serde::{Deserialize, Serialize};

/// Source of credential — where was it read from
#[derive(Debug, Clone, PartialEq)]
pub enum CredentialSourceType {
    EnvVar(String),
    ConfigFile,
    CustomProvider(String),
    EnvFile,
    Manual,
}

impl std::fmt::Display for CredentialSourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CredentialSourceType::EnvVar(v) => write!(f, "env:{}", v),
            CredentialSourceType::ConfigFile => write!(f, "config.yaml"),
            CredentialSourceType::CustomProvider(n) => write!(f, "custom:{}", n),
            CredentialSourceType::EnvFile => write!(f, ".env"),
            CredentialSourceType::Manual => write!(f, "manual"),
        }
    }
}

/// A resolved credential entry with source tracking
#[derive(Debug, Clone)]
pub struct CredentialEntry {
    pub provider_name: String,
    pub api_key: String,
    pub base_url: String,
    pub model: String,
    pub priority: u32,
    pub source: CredentialSourceType,
    pub is_active: bool,
}

impl CredentialEntry {
    pub fn to_gateway_config(&self) -> GatewayConfig {
        GatewayConfig {
            provider: self.provider_name.clone(),
            model: self.model.clone(),
            api_key: self.api_key.clone(),
            base_url: Some(self.base_url.clone()),
            ..Default::default()
        }
    }
}

/// Multi-source credential pool with auto-discovery
#[derive(Debug, Clone)]
pub struct SuperCredentialPool {
    pub entries: Vec<CredentialEntry>,
    pub active_index: usize,
}

impl SuperCredentialPool {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            active_index: 0,
        }
    }

    /// Load credentials from all sources
    pub fn load_all() -> Self {
        let mut pool = Self::new();

        // 1. Custom providers from config.yaml
        pool.load_from_config();

        // 2. Model section from config.yaml
        pool.load_from_model_config();

        // 3. Environment variables (DeepSeek, OpenAI, OpenRouter)
        pool.load_from_env("DEEPSEEK_API_KEY", "DEEPSEEK_BASE_URL", "deepseek", "deepseek-chat");
        pool.load_from_env("OPENAI_API_KEY", "OPENAI_BASE_URL", "openai", "gpt-4o");
        pool.load_from_env("OPENROUTER_API_KEY", "OPENROUTER_BASE_URL", "openrouter", "anthropic/claude-sonnet-4");

        // Sort by priority
        pool.entries.sort_by_key(|e| e.priority);

        pool
    }

    /// Load custom_providers from config.yaml
    fn load_from_config(&mut self) {
        let config_path = shellexpand::tilde("~/.hermes/config.yaml");
        let content = match std::fs::read_to_string(config_path.as_ref()) {
            Ok(c) => c,
            Err(_) => return,
        };
        let cfg: Result<hermes_core::HermesConfig, _> = serde_yaml::from_str(&content);
        let cfg = match cfg {
            Ok(c) => c,
            Err(_) => return,
        };

        for cp in &cfg.custom_providers {
            if cp.api_key.is_empty() {
                continue;
            }
            let name = cp.name.clone();
            self.entries.push(CredentialEntry {
                provider_name: name.clone(),
                api_key: cp.api_key.clone(),
                base_url: cp.base_url.clone(),
                model: cp.model.clone(),
                priority: self.entries.len() as u32,
                source: CredentialSourceType::CustomProvider(name),
                is_active: self.entries.is_empty(),
            });
        }
    }

    /// Load model.api_key from config
    fn load_from_model_config(&mut self) {
        let config_path = shellexpand::tilde("~/.hermes/config.yaml");
        let content = match std::fs::read_to_string(config_path.as_ref()) {
            Ok(c) => c,
            Err(_) => return,
        };
        let cfg: Result<hermes_core::HermesConfig, _> = serde_yaml::from_str(&content);
        let cfg = match cfg {
            Ok(c) => c,
            Err(_) => return,
        };

        if let Some(ak) = &cfg.model.api_key {
            if !ak.is_empty() {
                let base = cfg.model.base_url.clone()
                    .filter(|u| !u.is_empty())
                    .unwrap_or_else(|| "https://api.deepseek.com".to_string());
                let model = if cfg.model.default == "gpt-5.5" {
                    "deepseek-chat".to_string()
                } else {
                    cfg.model.default.clone()
                };
                self.entries.push(CredentialEntry {
                    provider_name: cfg.model.provider.clone(),
                    api_key: ak.clone(),
                    base_url: base,
                    model,
                    priority: self.entries.len() as u32,
                    source: CredentialSourceType::ConfigFile,
                    is_active: self.entries.is_empty(),
                });
            }
        }
    }

    /// Load from a single env var pair
    fn load_from_env(&mut self, key_var: &str, url_var: &str, provider: &str, default_model: &str) {
        let api_key = match std::env::var(key_var) {
            Ok(k) if !k.is_empty() => k,
            _ => return,
        };
        let base_url = std::env::var(url_var)
            .unwrap_or_else(|_| match provider {
                "deepseek" => "https://api.deepseek.com".to_string(),
                "openrouter" => "https://openrouter.ai/api/v1".to_string(),
                _ => "https://api.openai.com/v1".to_string(),
            });
        let model = std::env::var("HERMES_MODEL")
            .unwrap_or_else(|_| default_model.to_string());

        self.entries.push(CredentialEntry {
            provider_name: provider.to_string(),
            api_key,
            base_url,
            model,
            priority: self.entries.len() as u32,
            source: CredentialSourceType::EnvVar(format!("${}", key_var)),
            is_active: self.entries.is_empty(),
        });
    }

    /// Get active credential
    pub fn active(&self) -> Option<&CredentialEntry> {
        self.entries.get(self.active_index)
    }

    /// Switch to next credential (rotate)
    pub fn rotate(&mut self) -> Option<&CredentialEntry> {
        if self.entries.is_empty() {
            return None;
        }
        if let Some(entry) = self.entries.get_mut(self.active_index) {
            entry.is_active = false;
        }
        self.active_index = (self.active_index + 1) % self.entries.len();
        if let Some(entry) = self.entries.get_mut(self.active_index) {
            entry.is_active = true;
        }
        self.entries.get(self.active_index)
    }

    /// Find credential by provider name
    pub fn find(&self, provider: &str) -> Option<&CredentialEntry> {
        self.entries.iter().find(|e| e.provider_name == provider)
    }

    /// Provider names
    pub fn provider_names(&self) -> Vec<String> {
        self.entries.iter().map(|e| e.provider_name.clone()).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}
