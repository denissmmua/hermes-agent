use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::info;

use hermes_core::{AgentConfig, GatewayConfig, ToolRegistry};
use hermes_gateway::{LLMProvider, OpenAIProvider};
use hermes_agent::AgentRuntime;
use hermes_tools::{ShellTool, ReadFileTool, WriteFileTool, GrepTool, GitTool, WebFetchTool};

pub struct WebUIConfig {
    pub port: u16,
    pub host: String,
    pub model: String,
    pub api_key: String,
    pub base_url: String,
}

fn static_html() -> &'static str { include_str!("../static/index.html") }
fn static_css() -> &'static str { include_str!("../static/style.css") }
fn static_js() -> &'static str { include_str!("../static/app.js") }

fn http_resp(status: &str, ctype: &str, body: &str) -> String {
    format!("HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", status, ctype, body.len(), body)
}

async fn run_chat(prompt: &str, gateway: Arc<dyn LLMProvider>, model: &str) -> String {
    let mut tools = ToolRegistry::new();
    let tools_list: Vec<Arc<dyn hermes_core::Tool>> = vec![Arc::new(ShellTool),Arc::new(ReadFileTool),Arc::new(WriteFileTool),
              Arc::new(GrepTool),Arc::new(GitTool),Arc::new(WebFetchTool)];
    for t in tools_list {
        tools.register(t);
    }
    let mut rt = AgentRuntime::new(AgentConfig {
        name:"hermes".to_string(), model: model.to_string(),
        system_prompt:"You are Hermes, an AI coding agent. Be concise.".to_string(),
        ..Default::default()
    }, gateway);
    for n in tools.names() { if let Some(t) = tools.get(&n) { rt.register_tool(t); } }
    match rt.run_turn(prompt).await {
        Ok(r) => serde_json::to_string(&serde_json::json!({"response": r})).unwrap_or_default(),
        Err(e) => serde_json::to_string(&serde_json::json!({"error": e.to_string()})).unwrap_or_default(),
    }
}

pub async fn start_webui(cfg: WebUIConfig) -> anyhow::Result<()> {
    let listener = TcpListener::bind(format!("{}:{}", cfg.host, cfg.port)).await?;
    info!("Hermes Web UI: http://{}:{}", cfg.host, cfg.port);

    let gateway = Arc::new(OpenAIProvider::new(GatewayConfig {
        model: cfg.model.clone(), api_key: cfg.api_key.clone(),
        base_url: Some(cfg.base_url.clone()), ..Default::default()
    }));

    let has_key = !cfg.api_key.is_empty();
    let model = cfg.model;

    loop {
        let (mut stream, _) = listener.accept().await?;
        let gw = gateway.clone();
        let m = model.clone();

        tokio::spawn(async move {
            let mut buf = vec![0u8; 16384];
            let n = match stream.read(&mut buf).await { Ok(n) if n > 0 => n, _ => return };
            let req = String::from_utf8_lossy(&buf[..n]);
            let fl = req.lines().next().unwrap_or("");
            let parts: Vec<&str> = fl.split_whitespace().collect();
            if parts.len() < 2 { return; }
            let path = parts[1];

            let resp = match path {
                "/" => http_resp("200 OK", "text/html; charset=utf-8", static_html()),
                "/style.css" => http_resp("200 OK", "text/css; charset=utf-8", static_css()),
                "/app.js" => http_resp("200 OK", "application/javascript", static_js()),
                "/api/status" => {
                    let json = serde_json::json!({"name":"Hermes Agent Rust","version":"0.1.0","model":m,"status":"running","api_key":has_key});
                    let body = serde_json::to_string(&json).unwrap_or_default();
                    http_resp("200 OK", "application/json", &body)
                }
                "/api/chat" => {
                    let start = req.find("\r\n\r\n").map(|i| i + 4).unwrap_or(0);
                    let body_str = &req[start..];
                    let prompt = serde_json::from_str::<serde_json::Value>(body_str)
                        .ok().and_then(|v| v["prompt"].as_str().map(|s| s.to_string()))
                        .unwrap_or_default();
                    let body = run_chat(&prompt, gw, &m).await;
                    http_resp("200 OK", "application/json", &body)
                }
                _ => http_resp("404 Not Found", "text/plain", "Not Found"),
            };

            let _ = stream.write_all(resp.as_bytes()).await;
        });
    }
}
