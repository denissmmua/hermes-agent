use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};

use hermes_core::{AgentError, AgentResult};

/// MCP (Model Context Protocol) server connection
pub struct MCPServer {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    child: Option<MCPProcess>,
}

struct MCPProcess {
    stdin: ChildStdin,
    child: Child,
}

impl MCPServer {
    pub fn new(
        name: impl Into<String>,
        command: impl Into<String>,
        args: Vec<String>,
        env: HashMap<String, String>,
    ) -> Self {
        Self {
            name: name.into(),
            command: command.into(),
            args,
            env,
            child: None,
        }
    }

    pub async fn start(&mut self) -> AgentResult<()> {
        let mut cmd = Command::new(&self.command);
        cmd.args(&self.args)
            .envs(&self.env)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        let mut child = cmd
            .spawn()
            .map_err(|e| AgentError::Plugin(format!("MCP start error: {}", e)))?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| AgentError::Plugin("MCP no stdin".into()))?;

        self.child = Some(MCPProcess { stdin, child });
        Ok(())
    }

    pub async fn send_request(&mut self, method: &str, params: serde_json::Value) -> AgentResult<serde_json::Value> {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params,
        });

        if let Some(proc) = &mut self.child {
            let mut buf = serde_json::to_vec(&request)?;
            buf.push(b'\n');
            proc.stdin
                .write_all(&buf)
                .await
                .map_err(|e| AgentError::Plugin(format!("MCP write error: {}", e)))?;
            proc.stdin
                .flush()
                .await
                .map_err(|e| AgentError::Plugin(format!("MCP flush error: {}", e)))?;
        }

        // For now return a placeholder
        Ok(serde_json::json!({"status": "sent"}))
    }

    pub fn is_running(&self) -> bool {
        self.child.is_some()
    }

    pub async fn stop(&mut self) -> AgentResult<()> {
        if let Some(mut proc) = self.child.take() {
            let _ = proc.child.kill().await;
        }
        Ok(())
    }
}

pub struct MCPRegistry {
    servers: HashMap<String, Arc<tokio::sync::Mutex<MCPServer>>>,
}

impl MCPRegistry {
    pub fn new() -> Self {
        Self {
            servers: HashMap::new(),
        }
    }

    pub fn register(&mut self, server: MCPServer) {
        self.servers
            .insert(server.name.clone(), Arc::new(tokio::sync::Mutex::new(server)));
    }

    pub async fn start_all(&mut self) -> AgentResult<()> {
        for (_, server) in &self.servers {
            let mut srv = server.lock().await;
            if !srv.is_running() {
                srv.start().await?;
            }
        }
        Ok(())
    }

    pub fn get(&self, name: &str) -> Option<Arc<tokio::sync::Mutex<MCPServer>>> {
        self.servers.get(name).cloned()
    }
}
