use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::io::AsyncReadExt;
use tracing::info;

use hermes_core::GatewayConfig;
use hermes_gateway::{LLMProvider, OpenAIProvider};

pub struct DashboardConfig {
    pub port: u16,
    pub host: String,
    pub gateway_config: GatewayConfig,
}

impl Default for DashboardConfig {
    fn default() -> Self {
        Self {
            port: 9090,
            host: "0.0.0.0".to_string(),
            gateway_config: GatewayConfig::default(),
        }
    }
}

pub async fn start_dashboard(config: DashboardConfig) -> anyhow::Result<()> {
    let addr = format!("{}:{}", config.host, config.port);
    let listener = TcpListener::bind(&addr).await?;
    info!("📊 Hermes Dashboard: http://{}", addr);

    let gateway = Arc::new(OpenAIProvider::new(config.gateway_config));

    loop {
        let (mut stream, _) = listener.accept().await?;
        let gw = gateway.clone();

        tokio::spawn(async move {
            let (reader, mut writer) = stream.split();
            let mut buf_reader = BufReader::new(reader);
            let mut request_line = String::new();
            if buf_reader.read_line(&mut request_line).await.is_err() {
                return;
            }

            // Read headers
            let mut headers = HashMap::new();
            loop {
                let mut line = String::new();
                match buf_reader.read_line(&mut line).await {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {
                        if line.trim().is_empty() { break; }
                        if let Some((k, v)) = line.split_once(':') {
                            headers.insert(k.trim().to_lowercase(), v.trim().to_string());
                        }
                    }
                }
            }

            let get_path = request_line.split_whitespace().nth(1).unwrap_or("/");

            let response = match get_path {
                "/" => build_html_response(INDEX_HTML),
                "/api/chat" => {
                    // Read body
                    let mut body = String::new();
                    if let Some(len_str) = headers.get("content-length") {
                        if let Ok(len) = len_str.parse::<usize>() {
                            let mut buf = vec![0u8; len.min(65536)];
                            if let Ok(n) = buf_reader.read(&mut buf).await {
                                body = String::from_utf8_lossy(&buf[..n]).to_string();
                            }
                        }
                    }

                    // Quick API chat
                    let prompt = serde_json::from_str::<serde_json::Value>(&body)
                        .and_then(|v| Ok(v["prompt"].as_str().unwrap_or("").to_string()))
                        .unwrap_or_default();

                    let request = hermes_gateway::LLMRequest {
                        model: "deepseek-chat".to_string(),
                        messages: vec![
                            hermes_gateway::LLMMessage { role: "system".to_string(), content: "You are Hermes.".to_string() },
                            hermes_gateway::LLMMessage { role: "user".to_string(), content: prompt },
                        ],
                        max_tokens: Some(1024),
                        temperature: Some(0.7),
                        stop: None,
                        stream: false,
                        tools: Vec::new(),
                    };

                    match gw.chat(request).await {
                        Ok(response) => {
                            let json = serde_json::json!({"response": response.content});
                            build_json_response(&json)
                        }
                        Err(e) => build_json_response(&serde_json::json!({"error": e.to_string()})),
                    }
                }
                "/api/status" => {
                    let json = serde_json::json!({
                        "name": "Hermes Agent Rust",
                        "version": "0.1.0",
                        "status": "running"
                    });
                    build_json_response(&json)
                }
                _ => {
                    // Try to serve static files
                    let path = get_path.trim_start_matches('/');
                    let content = match path {
                        "style.css" => Some(CSS),
                        "app.js" => Some(JS),
                        _ => None,
                    };
                    match content {
                        Some(c) => build_response("200 OK", "text/css", c),
                        None => build_response("404 Not Found", "text/plain", "Not Found"),
                    }
                }
            };

            let _ = writer.write_all(response.as_bytes()).await;
        });
    }
}

fn build_response(status: &str, content_type: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        status, content_type, body.len(), body
    )
}

fn build_html_response(body: &str) -> String {
    build_response("200 OK", "text/html; charset=utf-8", body)
}

fn build_json_response(value: &serde_json::Value) -> String {
    let body = serde_json::to_string(value).unwrap_or_default();
    build_response("200 OK", "application/json", &body)
}

const INDEX_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Hermes Agent Rust</title>
<link rel="stylesheet" href="/style.css">
</head>
<body>
<div class="container">
  <header>
    <h1>🔷 Hermes Agent Rust</h1>
    <p class="subtitle">Port of NousResearch/hermes-agent — running in Rust</p>
    <div id="status" class="status">⏳ Connecting...</div>
  </header>

  <main>
    <section class="chat-section">
      <h2>Chat</h2>
      <div id="chat" class="chat-box"></div>
      <div class="input-row">
        <input type="text" id="prompt" placeholder="Ask something..." />
        <button onclick="send()">Send</button>
      </div>
    </section>

    <section class="tools-section">
      <h2>Tools</h2>
      <ul id="tools">
        <li>shell</li>
        <li>read_file</li>
        <li>write_file</li>
        <li>grep</li>
        <li>git</li>
        <li>web_fetch</li>
      </ul>
    </section>
  </main>
</div>

<script src="/app.js"></script>
</body>
</html>"#;

const CSS: &str = r#"* { margin: 0; padding: 0; box-sizing: border-box; }
body { font-family: -apple-system, system-ui, sans-serif; background: #0d1117; color: #c9d1d9; }
.container { max-width: 900px; margin: 0 auto; padding: 20px; }
header { margin-bottom: 24px; }
h1 { font-size: 1.5rem; margin-bottom: 4px; }
.subtitle { color: #8b949e; font-size: 0.85rem; }
.status { margin-top: 8px; padding: 4px 12px; border-radius: 4px; font-size: 0.8rem; display: inline-block; }
.status.online { background: #1a4731; color: #7ee787; }
.status.offline { background: #47201a; color: #ff7b72; }
.chat-box { background: #161b22; border: 1px solid #30363d; border-radius: 6px; padding: 16px; height: 400px; overflow-y: auto; margin-bottom: 12px; }
.chat-box .msg { margin-bottom: 12px; }
.chat-box .msg.user { color: #58a6ff; }
.chat-box .msg.assistant { color: #c9d1d9; }
.chat-box .msg .role { font-weight: 600; font-size: 0.8rem; color: #8b949e; margin-bottom: 2px; }
.input-row { display: flex; gap: 8px; }
input { flex: 1; padding: 10px 12px; background: #21262d; border: 1px solid #30363d; border-radius: 6px; color: #c9d1d9; font-size: 14px; }
button { padding: 10px 20px; background: #238636; border: none; border-radius: 6px; color: white; cursor: pointer; font-size: 14px; }
button:hover { background: #2ea043; }
.tools-section { margin-top: 24px; }
.tools-section ul { display: flex; gap: 8px; flex-wrap: wrap; margin-top: 8px; }
.tools-section li { list-style: none; padding: 4px 12px; background: #21262d; border: 1px solid #30363d; border-radius: 12px; font-size: 0.8rem; }
@media (max-width: 600px) { .container { padding: 12px; } }
"#;

const JS: &str = r#"
async function checkStatus() {
  try {
    const r = await fetch('/api/status');
    const d = await r.json();
    document.getElementById('status').textContent = '✅ ' + d.name + ' — online';
    document.getElementById('status').className = 'status online';
  } catch {
    document.getElementById('status').textContent = '❌ Offline';
    document.getElementById('status').className = 'status offline';
  }
}

async function send() {
  const input = document.getElementById('prompt');
  const prompt = input.value.trim();
  if (!prompt) return;
  input.value = '';
  const chat = document.getElementById('chat');
  chat.innerHTML += '<div class="msg user"><div class="role">You</div>' + escapeHtml(prompt) + '</div>';
  chat.innerHTML += '<div class="msg assistant"><div class="role">Hermes</div><em>thinking...</em></div>';
  chat.scrollTop = chat.scrollHeight;
  try {
    const r = await fetch('/api/chat', { method:'POST', body: JSON.stringify({prompt}) });
    const d = await r.json();
    chat.querySelector('.msg.assistant:last-child').innerHTML = '<div class="role">Hermes</div>' + escapeHtml(d.response || d.error);
  } catch(e) {
    chat.querySelector('.msg.assistant:last-child').innerHTML = '<div class="role">Hermes</div>Error: ' + e;
  }
  chat.scrollTop = chat.scrollHeight;
}

function escapeHtml(s) {
  return s.replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;');
}

checkStatus();
setInterval(checkStatus, 30000);
"#;
