use async_trait::async_trait;
use hermes_core::{AgentError, AgentResult, Tool, ToolDefinition, ToolInput, ToolOutput};
use std::time::{Duration, Instant};
use tokio::time::timeout;

pub struct ShellTool;

#[async_trait]
impl Tool for ShellTool {
    fn name(&self) -> &str {
        "shell"
    }

    fn description(&self) -> &str {
        "Execute a shell command and return its output"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The shell command to execute"
                    },
                    "timeout": {
                        "type": "integer",
                        "description": "Timeout in seconds",
                        "default": 30
                    }
                },
                "required": ["command"]
            }),
        }
    }

    async fn execute(&self, input: ToolInput) -> AgentResult<ToolOutput> {
        let cmd = input.arguments["command"]
            .as_str()
            .ok_or_else(|| AgentError::Tool("Missing 'command' argument".into()))?;

        let timeout_secs = input.arguments["timeout"].as_u64().unwrap_or(30);
        let start = Instant::now();

        let child = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| AgentError::Tool(format!("Shell spawn error: {}", e)))?;

        let result = timeout(Duration::from_secs(timeout_secs), child.wait_with_output()).await;

        match result {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let combined = if stderr.is_empty() {
                    stdout
                } else {
                    format!("stdout:\n{}\nstderr:\n{}", stdout, stderr)
                };
                Ok(ToolOutput {
                    success: output.status.success(),
                    output: combined,
                    duration_ms: start.elapsed().as_millis() as u64,
                })
            }
            Ok(Err(e)) => Err(AgentError::Tool(format!("Shell execution error: {}", e))),
            Err(_) => Err(AgentError::Tool(format!(
                "Command timed out after {}s",
                timeout_secs
            ))),
        }
    }
}
