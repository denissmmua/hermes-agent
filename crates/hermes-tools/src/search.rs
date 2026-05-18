use async_trait::async_trait;
use hermes_core::{AgentError, AgentResult, Tool, ToolDefinition, ToolInput, ToolOutput};
use std::time::Instant;

pub struct GlobTool;

#[async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &str {
        "glob"
    }

    fn description(&self) -> &str {
        "Search for files matching a glob pattern"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "Glob pattern (e.g. '**/*.rs')"
                    }
                },
                "required": ["pattern"]
            }),
        }
    }

    async fn execute(&self, input: ToolInput) -> AgentResult<ToolOutput> {
        let pattern = input.arguments["pattern"]
            .as_str()
            .ok_or_else(|| AgentError::Tool("Missing 'pattern'".into()))?;

        let start = Instant::now();
        // Use tokio::process::Command with fd
        let child = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(format!("find . -path '{}' -type f 2>/dev/null | head -200", pattern))
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| AgentError::Tool(format!("Glob error: {}", e)))?;

        let output = child.wait_with_output().await
            .map_err(|e| AgentError::Tool(format!("Glob error: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let count = stdout.lines().count();

        Ok(ToolOutput {
            success: true,
            output: format!("Found {} files:\n{}", count, stdout),
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }
}

pub struct GrepTool;

#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &str {
        "grep"
    }

    fn description(&self) -> &str {
        "Search for text in files using ripgrep or grep"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": { "type": "string", "description": "Text pattern to search" },
                    "path": { "type": "string", "description": "Directory to search (default: .)", "default": "." }
                },
                "required": ["pattern"]
            }),
        }
    }

    async fn execute(&self, input: ToolInput) -> AgentResult<ToolOutput> {
        let pattern = input.arguments["pattern"]
            .as_str()
            .ok_or_else(|| AgentError::Tool("Missing 'pattern'".into()))?;

        let path = input.arguments["path"].as_str().unwrap_or(".");
        let start = Instant::now();

        // Try ripgrep first, fallback to grep
        let cmd = if which_rs("rg").await {
            format!("rg --no-heading -n '{}' '{}' 2>/dev/null | head -200", pattern, path)
        } else {
            format!("grep -rn '{}' '{}' 2>/dev/null | head -200", pattern, path)
        };

        let child = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(&cmd)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| AgentError::Tool(format!("Grep error: {}", e)))?;

        let output = child.wait_with_output().await
            .map_err(|e| AgentError::Tool(format!("Grep error: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        Ok(ToolOutput {
            success: true,
            output: stdout,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }
}

async fn which_rs(cmd: &str) -> bool {
    tokio::process::Command::new("which")
        .arg(cmd)
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub struct WebFetchTool;

#[async_trait]
impl Tool for WebFetchTool {
    fn name(&self) -> &str {
        "web_fetch"
    }

    fn description(&self) -> &str {
        "Fetch a URL and return its content"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "URL to fetch" },
                    "max_bytes": { "type": "integer", "description": "Max bytes to read", "default": 100000 }
                },
                "required": ["url"]
            }),
        }
    }

    async fn execute(&self, input: ToolInput) -> AgentResult<ToolOutput> {
        let url = input.arguments["url"]
            .as_str()
            .ok_or_else(|| AgentError::Tool("Missing 'url'".into()))?;

        let max_bytes = input.arguments["max_bytes"].as_u64().unwrap_or(100000) as usize;
        let start = Instant::now();

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .map_err(|e| AgentError::Tool(format!("HTTP client error: {}", e)))?;

        let response = client
            .get(url)
            .header("User-Agent", "Hermes-Agent-Rust/0.1")
            .send()
            .await
            .map_err(|e| AgentError::Tool(format!("HTTP error: {}", e)))?;

        let status = response.status();
        let text = response.text().await
            .map_err(|e| AgentError::Tool(format!("Read error: {}", e)))?;

        let truncated = if text.len() > max_bytes {
            format!("{}...[truncated {} bytes]", &text[..max_bytes], text.len() - max_bytes)
        } else {
            text
        };

        Ok(ToolOutput {
            success: status.is_success(),
            output: format!("Status: {}\n\n{}", status, truncated),
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }
}
