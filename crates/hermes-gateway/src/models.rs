// 1:1 port of model_tools.py - model catalog, pricing, fallback
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ModelInfo {
    pub id: String,
    pub provider: String,
    pub display_name: String,
    pub context_window: u32,
    pub max_output: u32,
    pub supports_tools: bool,
    pub supports_streaming: bool,
    pub supports_vision: bool,
    pub input_price_per_1k: f64,
    pub output_price_per_1k: f64,
}

pub fn default_model_catalog() -> HashMap<String, ModelInfo> {
    let mut m = HashMap::new();
    let models = vec![
        ModelInfo {
            id: "deepseek-chat".into(), provider: "deepseek".into(),
            display_name: "DeepSeek Chat".into(), context_window: 128_000,
            max_output: 8_192, supports_tools: true, supports_streaming: true,
            supports_vision: true, input_price_per_1k: 0.00027, output_price_per_1k: 0.0011,
        },
        ModelInfo {
            id: "deepseek-v4-flash".into(), provider: "deepseek".into(),
            display_name: "DeepSeek V4 Flash".into(), context_window: 128_000,
            max_output: 8_192, supports_tools: true, supports_streaming: true,
            supports_vision: true, input_price_per_1k: 0.00014, output_price_per_1k: 0.00056,
        },
        ModelInfo {
            id: "deepseek-v4-pro".into(), provider: "deepseek".into(),
            display_name: "DeepSeek V4 Pro".into(), context_window: 256_000,
            max_output: 16_384, supports_tools: true, supports_streaming: true,
            supports_vision: true, input_price_per_1k: 0.00055, output_price_per_1k: 0.0022,
        },
        ModelInfo {
            id: "gpt-4o".into(), provider: "openai".into(),
            display_name: "GPT-4o".into(), context_window: 128_000,
            max_output: 16_384, supports_tools: true, supports_streaming: true,
            supports_vision: true, input_price_per_1k: 0.0025, output_price_per_1k: 0.01,
        },
        ModelInfo {
            id: "claude-sonnet-4".into(), provider: "anthropic".into(),
            display_name: "Claude Sonnet 4".into(), context_window: 200_000,
            max_output: 8_192, supports_tools: true, supports_streaming: true,
            supports_vision: true, input_price_per_1k: 0.003, output_price_per_1k: 0.015,
        },
    ];
    for model in models {
        m.insert(model.id.clone(), model);
    }
    m
}

pub struct ModelSelector {
    catalog: HashMap<String, ModelInfo>,
    providers: HashMap<String, Vec<String>>,
}

impl ModelSelector {
    pub fn new() -> Self {
        let catalog = default_model_catalog();
        let mut providers: HashMap<String, Vec<String>> = HashMap::new();
        for (id, info) in &catalog {
            providers.entry(info.provider.clone())
                .or_default()
                .push(id.clone());
        }
        Self { catalog, providers }
    }

    pub fn get(&self, id: &str) -> Option<&ModelInfo> {
        self.catalog.get(id)
    }

    pub fn all(&self) -> Vec<&ModelInfo> {
        self.catalog.values().collect()
    }

    pub fn by_provider(&self, provider: &str) -> Vec<&ModelInfo> {
        self.catalog.values()
            .filter(|m| m.provider == provider)
            .collect()
    }

    pub fn supports_tools(&self, id: &str) -> bool {
        self.catalog.get(id).map(|m| m.supports_tools).unwrap_or(false)
    }

    pub fn providers(&self) -> Vec<&str> {
        self.providers.keys().map(|s| s.as_str()).collect()
    }
}
