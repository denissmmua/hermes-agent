use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayConfig {
    pub provider: String,
    pub model: String,
    pub api_key: String,
    pub base_url: Option<String>,
    pub max_retries: u32,
    pub timeout_secs: u64,
}

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
pub struct HermesConfig {
    pub gateway: GatewayConfig,
    pub agent_name: String,
    pub tools_dir: Option<String>,
    pub skills_dir: Option<String>,
    pub state_dir: Option<String>,
}

impl Default for HermesConfig {
    fn default() -> Self {
        Self {
            gateway: GatewayConfig::default(),
            agent_name: "hermes".to_string(),
            tools_dir: None,
            skills_dir: None,
            state_dir: None,
        }
    }
}
