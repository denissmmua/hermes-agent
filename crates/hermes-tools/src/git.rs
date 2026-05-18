use async_trait::async_trait;
use hermes_core::{AgentError, AgentResult, Tool, ToolDefinition, ToolInput, ToolOutput};
use std::time::Instant;

pub struct GitTool;

#[async_trait]
impl Tool for GitTool {
    fn name(&self) -> &str {
        "git"
    }

    fn description(&self) -> &str {
        "Run a git command in the current repository"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "args": {
                        "type": "string",
                        "description": "Git arguments (e.g. 'log --oneline -5')"
                    }
                },
                "required": ["args"]
            }),
        }
    }

    async fn execute(&self, input: ToolInput) -> AgentResult<ToolOutput> {
        let args = input.arguments["args"]
            .as_str()
            .ok_or_else(|| AgentError::Tool("Missing 'args'".into()))?;

        let start = Instant::now();

        let child = tokio::process::Command::new("git")
            .arg("--no-pager")
            .args(args.split_whitespace())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| AgentError::Tool(format!("Git error: {}", e)))?;

        let output = child.wait_with_output().await
            .map_err(|e| AgentError::Tool(format!("Git error: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let combined = if stderr.is_empty() { stdout } else { format!("{}\n{}", stdout, stderr) };

        Ok(ToolOutput {
            success: output.status.success(),
            output: combined,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }
}
