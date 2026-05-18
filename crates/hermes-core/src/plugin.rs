use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use crate::error::AgentResult;

/// Plugin capability: what the plugin can do
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PluginCapability {
    /// Hook into message processing
    MessageHook,
    /// Hook into tool execution
    ToolHook,
    /// Provide custom tools
    ToolProvider,
    /// Provide custom skills
    SkillProvider,
    /// Handle events
    EventHandler,
    /// Custom
    Custom(String),
}

/// Metadata describing a plugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMeta {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: Option<String>,
    pub capabilities: Vec<PluginCapability>,
    pub dependencies: Vec<String>,
    pub config_schema: Option<serde_json::Value>,
}

/// Context passed to plugin hooks
#[derive(Debug, Clone)]
pub struct PluginContext {
    pub agent_name: String,
    pub config: serde_json::Value,
    pub state: Option<serde_json::Value>,
}

/// Result from a plugin hook
#[derive(Debug, Clone)]
pub struct PluginResult {
    pub modified: bool,
    pub data: Option<serde_json::Value>,
    pub error: Option<String>,
}

impl PluginResult {
    pub fn passthrough() -> Self {
        Self { modified: false, data: None, error: None }
    }

    pub fn modified(data: serde_json::Value) -> Self {
        Self { modified: true, data: Some(data), error: None }
    }
}

/// Core trait for all plugins
#[async_trait]
pub trait Plugin: Send + Sync {
    fn meta(&self) -> &PluginMeta;

    /// Initialize the plugin with configuration
    async fn init(&self, ctx: &PluginContext) -> AgentResult<()> {
        let _ = ctx;
        Ok(())
    }

    /// Hook called before a message is sent to the LLM
    async fn pre_process_message(&self, _ctx: &PluginContext, _message: &mut crate::Message) -> AgentResult<PluginResult> {
        Ok(PluginResult::passthrough())
    }

    /// Hook called after LLM response
    async fn post_process_response(&self, _ctx: &PluginContext, _response: &mut serde_json::Value) -> AgentResult<PluginResult> {
        Ok(PluginResult::passthrough())
    }

    /// Hook called before tool execution
    async fn pre_tool_execute(&self, _ctx: &PluginContext, _tool_name: &str, _args: &mut serde_json::Value) -> AgentResult<PluginResult> {
        Ok(PluginResult::passthrough())
    }

    /// Hook called after tool execution
    async fn post_tool_execute(&self, _ctx: &PluginContext, _tool_name: &str, _output: &mut crate::ToolOutput) -> AgentResult<PluginResult> {
        Ok(PluginResult::passthrough())
    }

    /// Shutdown hook
    async fn shutdown(&self) -> AgentResult<()> {
        Ok(())
    }
}

pub type PluginBox = Arc<dyn Plugin>;

/// Registry for managing plugins
pub struct PluginRegistry {
    plugins: HashMap<String, PluginBox>,
    configs: HashMap<String, serde_json::Value>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self { plugins: HashMap::new(), configs: HashMap::new() }
    }

    pub fn register(&mut self, plugin: PluginBox, config: Option<serde_json::Value>) {
        let name = plugin.meta().name.clone();
        self.plugins.insert(name.clone(), plugin);
        if let Some(cfg) = config {
            self.configs.insert(name, cfg);
        }
    }

    pub fn get(&self, name: &str) -> Option<PluginBox> {
        self.plugins.get(name).cloned()
    }

    pub fn names(&self) -> Vec<String> {
        self.plugins.keys().cloned().collect()
    }

    pub fn is_empty(&self) -> bool { self.plugins.is_empty() }
    pub fn len(&self) -> usize { self.plugins.len() }

    pub fn get_config(&self, name: &str) -> Option<serde_json::Value> {
        self.configs.get(name).cloned()
    }

    /// Initialize all registered plugins
    pub async fn init_all(&self, agent_name: &str) -> AgentResult<()> {
        for (name, plugin) in &self.plugins {
            let ctx = PluginContext {
                agent_name: agent_name.to_string(),
                config: self.configs.get(name).cloned().unwrap_or(serde_json::Value::Null),
                state: None,
            };
            plugin.init(&ctx).await?;
        }
        Ok(())
    }

    /// Shutdown all plugins
    pub async fn shutdown_all(&self) -> AgentResult<()> {
        for plugin in self.plugins.values() {
            plugin.shutdown().await?;
        }
        Ok(())
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}
