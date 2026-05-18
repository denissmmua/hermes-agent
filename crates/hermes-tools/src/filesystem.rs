use async_trait::async_trait;
use hermes_core::{AgentResult, Tool, ToolDefinition, ToolInput, ToolOutput};
use std::time::Instant;

pub struct ReadFileTool;

#[async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }

    fn description(&self) -> &str {
        "Read the contents of a file"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Absolute path to the file"
                    }
                },
                "required": ["path"]
            }),
        }
    }

    async fn execute(&self, input: ToolInput) -> AgentResult<ToolOutput> {
        let path = input.arguments["path"]
            .as_str()
            .ok_or_else(|| hermes_core::AgentError::Tool("Missing 'path' argument".into()))?;

        let start = Instant::now();
        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| hermes_core::AgentError::Io(e))?;

        Ok(ToolOutput {
            success: true,
            output: content,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }
}

pub struct WriteFileTool;

#[async_trait]
impl Tool for WriteFileTool {
    fn name(&self) -> &str {
        "write_file"
    }

    fn description(&self) -> &str {
        "Write content to a file"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Absolute path" },
                    "content": { "type": "string", "description": "Content to write" }
                },
                "required": ["path", "content"]
            }),
        }
    }

    async fn execute(&self, input: ToolInput) -> AgentResult<ToolOutput> {
        let path = input.arguments["path"]
            .as_str()
            .ok_or_else(|| hermes_core::AgentError::Tool("Missing 'path'".into()))?;
        let content = input.arguments["content"]
            .as_str()
            .ok_or_else(|| hermes_core::AgentError::Tool("Missing 'content'".into()))?;

        let start = Instant::now();
        tokio::fs::write(path, content)
            .await
            .map_err(|e| hermes_core::AgentError::Io(e))?;

        Ok(ToolOutput {
            success: true,
            output: format!("Written {} bytes to {}", content.len(), path),
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }
}
