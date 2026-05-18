// 1:1 port of toolsets.py - toolset registration and grouping
use std::sync::Arc;
use hermes_core::tool::{Tool, ToolRegistry, ToolDefinition};

pub fn register_toolsets(registry: &mut ToolRegistry, toolset_names: &[String]) {
    for name in toolset_names {
        match name.as_str() {
            "hermes-cli" => register_cli_tools(registry),
            "hermes-dev" => register_dev_tools(registry),
            "hermes-web" => register_web_tools(registry),
            _ => {}
        }
    }
}

fn register_cli_tools(registry: &mut ToolRegistry) {
    registry.register(Arc::new(super::ShellTool));
    registry.register(Arc::new(super::ReadFileTool));
    registry.register(Arc::new(super::WriteFileTool));
    registry.register(Arc::new(super::GrepTool));
    registry.register(Arc::new(super::GitTool));
    registry.register(Arc::new(super::WebFetchTool));
}

fn register_dev_tools(registry: &mut ToolRegistry) {
    registry.register(Arc::new(super::GrepTool));
    registry.register(Arc::new(super::GitTool));
}

fn register_web_tools(registry: &mut ToolRegistry) {
    registry.register(Arc::new(super::WebFetchTool));
}

pub fn get_tool_definitions(registry: &ToolRegistry) -> Vec<ToolDefinition> {
    let mut defs = Vec::new();
    for name in registry.names() {
        if let Some(tool) = registry.get(&name) {
            defs.push(tool.definition());
        }
    }
    defs
}
