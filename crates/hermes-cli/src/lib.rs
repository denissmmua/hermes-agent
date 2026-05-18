use std::sync::Arc;
use hermes_core::*;
use hermes_gateway::OpenAIProvider;
use tracing::info;

pub struct AgentLauncher {
    pub config: HermesConfig,
    pub state_db: Option<SessionDB>,
}

impl AgentLauncher {
    pub fn new(config: HermesConfig) -> Self {
        let db = config.state_dir.as_ref()
            .and_then(|dir| SessionDB::open(format!("{}/sessions.db", dir)).ok());
        Self { config, state_db: db }
    }

    pub fn gateway_from_config(cfg: &HermesConfig) -> Arc<dyn hermes_gateway::LLMProvider> {
        let gw = cfg.gateway.clone().unwrap_or_default();
        Arc::new(OpenAIProvider::new(gw))
    }
}
