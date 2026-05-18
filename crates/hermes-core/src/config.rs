use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayConfig {
    pub provider: String,
    pub model: String,
    #[serde(default)]
    pub api_key: String,
    pub base_url: Option<String>,
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
}

fn default_max_retries() -> u32 { 3 }
fn default_timeout() -> u64 { 120 }

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            provider: "openai".to_string(),
            model: "deepseek-v4-flash".to_string(),
            api_key: String::new(),
            base_url: None,
            max_retries: 3,
            timeout_secs: 120,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomProviderConfig {
    pub name: String,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentModelConfig {
    pub default: String,
    pub provider: String,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
}

impl Default for AgentModelConfig {
    fn default() -> Self {
        Self {
            default: "deepseek-v4-flash".to_string(),
            provider: "openai".to_string(),
            base_url: None,
            api_key: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSectionConfig {
    #[serde(default = "default_max_turns2")]
    pub max_turns: u32,
    #[serde(default = "default_gateway_timeout2")]
    pub gateway_timeout: u64,
    #[serde(default = "default_api_max_retries2")]
    pub api_max_retries: u32,
    #[serde(default)]
    pub verbose: bool,
}

fn default_max_turns2() -> u32 { 90 }
fn default_gateway_timeout2() -> u64 { 1800 }
fn default_api_max_retries2() -> u32 { 3 }

impl Default for AgentSectionConfig {
    fn default() -> Self {
        Self {
            max_turns: 90,
            gateway_timeout: 1800,
            api_max_retries: 3,
            verbose: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformConfig {
    pub enabled: Option<bool>,
    pub extra: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HermesConfig {
    #[serde(default)]
    pub model: AgentModelConfig,
    #[serde(default)]
    pub providers: serde_json::Value,
    #[serde(default)]
    pub fallback_providers: Vec<serde_json::Value>,
    #[serde(default)]
    pub credential_pool_strategies: serde_json::Value,
    #[serde(default)]
    pub toolsets: Vec<String>,
    #[serde(default)]
    pub agent: AgentSectionConfig,
    #[serde(default)]
    pub custom_providers: Vec<CustomProviderConfig>,
    #[serde(default)]
    pub platforms: std::collections::HashMap<String, PlatformConfig>,
    #[serde(default)]
    pub memory: serde_json::Value,
    #[serde(default)]
    pub cron: serde_json::Value,
    pub gateway: Option<GatewayConfig>,
    pub agent_name: Option<String>,
}

impl Default for HermesConfig {
    fn default() -> Self {
        Self {
            model: AgentModelConfig::default(),
            providers: serde_json::Value::Null,
            fallback_providers: Vec::new(),
            credential_pool_strategies: serde_json::Value::Null,
            toolsets: vec!["hermes-cli".to_string()],
            agent: AgentSectionConfig::default(),
            custom_providers: Vec::new(),
            platforms: std::collections::HashMap::new(),
            memory: serde_json::json!({"memory_enabled": true, "char_limit": 2200}),
            cron: serde_json::json!({"enabled": false}),
            gateway: None,
            agent_name: Some("hermes".to_string()),
        }
    }
}

impl HermesConfig {
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, crate::AgentError> {
        let content = std::fs::read_to_string(path.as_ref())?;
        let config: HermesConfig = serde_yaml::from_str(&content)
            .map_err(|e| crate::AgentError::Config(format!("YAML parse error: {}", e)))?;
        Ok(config)
    }

    pub fn to_file(&self, path: impl AsRef<Path>) -> Result<(), crate::AgentError> {
        let content = serde_yaml::to_string(self)
            .map_err(|e| crate::AgentError::Config(format!("YAML serialize error: {}", e)))?;
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path.as_ref(), &content)?;
        Ok(())
    }
}
