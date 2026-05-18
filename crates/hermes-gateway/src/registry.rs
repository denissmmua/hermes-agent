use std::collections::HashMap;
use std::sync::Arc;

use hermes_core::{GatewayConfig};

use crate::{LLMProvider, OpenAIProvider};

pub struct ProviderRegistry {
    providers: HashMap<String, Arc<dyn LLMProvider>>,
    default_provider: String,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
            default_provider: "openai".to_string(),
        }
    }

    pub fn register(&mut self, name: &str, provider: Arc<dyn LLMProvider>) {
        self.providers.insert(name.to_string(), provider);
    }

    pub fn register_openai(&mut self, config: GatewayConfig) {
        let name = config.provider.clone();
        self.providers
            .insert(name.clone(), Arc::new(OpenAIProvider::new(config)));
        self.default_provider = name;
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn LLMProvider>> {
        self.providers.get(name).cloned()
    }

    pub fn default(&self) -> Option<Arc<dyn LLMProvider>> {
        self.get(&self.default_provider)
    }

    pub fn set_default(&mut self, name: &str) {
        self.default_provider = name.to_string();
    }

    pub fn list_providers(&self) -> Vec<String> {
        self.providers.keys().cloned().collect()
    }
}
