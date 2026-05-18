use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

use hermes_core::AgentResult;
use hermes_agent::AIAgent;

// ═══════════════════════════════════════════════════════════════════════
// Agent Cache
// ═══════════════════════════════════════════════════════════════════════

struct CachedAgent {
    agent: AIAgent,
    last_used: Instant,
    session_id: String,
    platform: String,
}

pub struct AgentCache {
    agents: HashMap<String, CachedAgent>,
    max_size: usize,
    idle_ttl: Duration,
}

impl AgentCache {
    pub fn new(max_size: usize, idle_ttl_secs: f64) -> Self {
        Self {
            agents: HashMap::new(),
            max_size,
            idle_ttl: Duration::from_secs_f64(idle_ttl_secs),
        }
    }

    pub fn get(&mut self, session_id: &str) -> Option<&mut AIAgent> {
        if let Some(cached) = self.agents.get_mut(session_id) {
            cached.last_used = Instant::now();
            Some(&mut cached.agent)
        } else {
            None
        }
    }

    pub fn insert(&mut self, session_id: String, agent: AIAgent, platform: &str) {
        self.evict_idle();
        if self.agents.len() >= self.max_size {
            // Evict oldest
            let oldest = self.agents.iter()
                .min_by_key(|(_, a)| a.last_used)
                .map(|(k, _)| k.clone());
            if let Some(key) = oldest {
                self.agents.remove(&key);
            }
        }
        self.agents.insert(session_id.clone(), CachedAgent {
            agent,
            last_used: Instant::now(),
            session_id: session_id.clone(),
            platform: platform.to_string(),
        });
    }

    fn evict_idle(&mut self) {
        let cutoff = Instant::now() - self.idle_ttl;
        self.agents.retain(|_, a| a.last_used > cutoff);
    }

    pub fn len(&self) -> usize { self.agents.len() }
}

// ═══════════════════════════════════════════════════════════════════════
// Gateway Request / Response
// ═══════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, serde::Deserialize)]
pub struct GatewayRequest {
    pub platform: String,
    pub chat_id: String,
    pub user_id: String,
    pub user_name: String,
    pub text: String,
    pub session_id: Option<String>,
    pub message_id: Option<String>,
    pub thread_id: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct GatewayResponse {
    pub success: bool,
    pub response: Option<String>,
    pub session_id: Option<String>,
    pub error: Option<String>,
}

// ═══════════════════════════════════════════════════════════════════════
// Platform Handler Trait
// ═══════════════════════════════════════════════════════════════════════

#[async_trait::async_trait]
pub trait PlatformAdapter: Send + Sync {
    fn name(&self) -> &str;
    async fn handle_message(&self, req: &GatewayRequest) -> GatewayResponse;
    async fn send_typing(&self, chat_id: &str) -> AgentResult<()>;
}

// ═══════════════════════════════════════════════════════════════════════
// Main Gateway
// ═══════════════════════════════════════════════════════════════════════

pub struct Gateway {
    pub port: u16,
    pub host: String,
    pub agent_cache: Arc<Mutex<AgentCache>>,
    pub platforms: HashMap<String, Arc<dyn PlatformAdapter>>,
    pub running: Arc<Mutex<bool>>,

    // Webhook-based platforms: Telegram, Discord, Slack send POST to us
    pub webhook_secrets: HashMap<String, String>,
}

impl Gateway {
    pub fn new(port: u16, host: &str) -> Self {
        Self {
            port,
            host: host.to_string(),
            agent_cache: Arc::new(Mutex::new(AgentCache::new(128, 3600.0))),
            platforms: HashMap::new(),
            running: Arc::new(Mutex::new(false)),
            webhook_secrets: HashMap::new(),
        }
    }

    pub fn register_platform(&mut self, adapter: Arc<dyn PlatformAdapter>) {
        self.platforms.insert(adapter.name().to_string(), adapter);
    }

    pub fn set_webhook_secret(&mut self, platform: &str, secret: &str) {
        self.webhook_secrets.insert(platform.to_string(), secret.to_string());
    }

    /// Start the gateway server (HTTP + platform adapters)
    pub async fn start(&self) -> AgentResult<()> {
        {
            let mut running = self.running.lock().await;
            *running = true;
        }

        let addr = format!("{}:{}", self.host, self.port);
        let listener = TcpListener::bind(&addr).await
            .map_err(|e| hermes_core::AgentError::Config(format!("Bind {}: {}", addr, e)))?;

        info!("🚀 Gateway listening on {}", addr);
        info!("  Platforms: {}", self.platforms.keys().cloned().collect::<Vec<_>>().join(", "));

        let agent_cache = self.agent_cache.clone();
        let platforms = self.platforms.clone();

        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((mut socket, addr)) => {
                        let agent_cache = agent_cache.clone();
                        let platforms = platforms.clone();
                        tokio::spawn(async move {
                            use tokio::io::{AsyncReadExt, AsyncWriteExt};

                            let mut buf = vec![0u8; 8192];
                            let n = match socket.read(&mut buf).await {
                                Ok(n) if n > 0 => n,
                                _ => return,
                            };

                            let request = String::from_utf8_lossy(&buf[..n]).to_string();

                            // Parse HTTP
                            let (method, path, body) = parse_http_request(&request);

                            let response = match (method.as_str(), path.as_str()) {
                                ("POST", "/webhook") => {
                                    handle_webhook(&body, &agent_cache, &platforms).await
                                }
                                ("GET", "/health") => {
                                    health_response()
                                }
                                ("GET", "/status") => {
                                    let count = agent_cache.lock().await.len();
                                    format!(r#"{{"status":"ok","cached_agents":{}}}"#, count)
                                }
                                _ => {
                                    format!("HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n")
                                }
                            };

                            let _ = socket.write_all(response.as_bytes()).await;
                        });
                    }
                    Err(e) => error!("Accept: {}", e),
                }
            }
        });

        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// HTTP request parser
// ═══════════════════════════════════════════════════════════════════════

fn parse_http_request(request: &str) -> (String, String, String) {
    let mut lines = request.lines();
    let request_line = lines.next().unwrap_or("");
    let parts: Vec<&str> = request_line.split_whitespace().collect();
    let method = parts.first().unwrap_or(&"").to_string();
    let path = parts.get(1).unwrap_or(&"").to_string();

    // Find body after blank line
    let body = if let Some(pos) = request.find("\r\n\r\n") {
        request[pos + 4..].to_string()
    } else if let Some(pos) = request.find("\n\n") {
        request[pos + 2..].to_string()
    } else {
        String::new()
    };

    (method, path, body)
}

// ═══════════════════════════════════════════════════════════════════════
// Webhook handler
// ═══════════════════════════════════════════════════════════════════════

async fn handle_webhook(
    body: &str,
    agent_cache: &Arc<Mutex<AgentCache>>,
    platforms: &HashMap<String, Arc<dyn PlatformAdapter>>,
) -> String {
    // Parse JSON body
    let req: GatewayRequest = match serde_json::from_str(body) {
        Ok(r) => r,
        Err(e) => {
            return json_response(false, Some(&format!("Parse error: {}", e)), None);
        }
    };

    // Find platform handler
    let platform = match platforms.get(&req.platform) {
        Some(p) => p.clone(),
        None => {
            // Try default handler
            return json_response(false, Some(&format!("Unknown platform: {}", req.platform)), None);
        }
    };

    let response = platform.handle_message(&req).await;
    json_response(response.success, response.response.as_deref(), response.session_id.as_deref())
}

fn json_response(success: bool, response: Option<&str>, session_id: Option<&str>) -> String {
    let mut parts = Vec::new();
    parts.push(format!(r#""success":{}"#, success));
    if let Some(r) = response {
        let escaped = serde_json::to_string(r).unwrap_or_else(|_| r.to_string());
        parts.push(format!(r#""response":{}"#, escaped));
    }
    if let Some(s) = session_id {
        parts.push(format!(r#""session_id":"{}""#, s));
    }
    let json = format!("{{{}}}", parts.join(","));
    format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}", json.len(), json)
}

fn health_response() -> String {
    let body = r#"{"status":"ok"}"#;
    format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}", body.len(), body)
}

// ═══════════════════════════════════════════════════════════════════════
// Generic platform adapter — handles any LLM-backed chat platform
// ═══════════════════════════════════════════════════════════════════════

pub struct GenericPlatformAdapter {
    pub name: String,
    pub agent_cache: Arc<Mutex<AgentCache>>,
}

#[async_trait::async_trait]
impl PlatformAdapter for GenericPlatformAdapter {
    fn name(&self) -> &str { &self.name }

    async fn handle_message(&self, req: &GatewayRequest) -> GatewayResponse {
        let session_id = req.session_id.clone()
            .unwrap_or_else(|| format!("gw:{}:{}", req.platform, req.chat_id));

        let mut cache = self.agent_cache.lock().await;
        let agent = match cache.get(&session_id) {
            Some(a) => a,
            None => {
                let mut new_agent = AIAgent::new();
                new_agent.session.id = session_id.clone();
                new_agent.session.platform = req.platform.clone();
                new_agent.session.user_id = req.user_id.clone();
                new_agent.session.user_name = req.user_name.clone();
                new_agent.session.chat_id = req.chat_id.clone();

                // Register default tools
                let tools: Vec<Arc<dyn hermes_core::Tool>> = vec![
                    Arc::new(hermes_tools::ShellTool),
                    Arc::new(hermes_tools::ReadFileTool),
                    Arc::new(hermes_tools::WriteFileTool),
                    Arc::new(hermes_tools::GrepTool),
                    Arc::new(hermes_tools::GitTool),
                    Arc::new(hermes_tools::WebFetchTool),
                ];
                new_agent.register_tools(tools);

                cache.insert(session_id.clone(), new_agent, &req.platform);
                cache.get(&session_id).unwrap()
            }
        };

        match agent.run_turn(&req.text).await {
            Ok(response) => GatewayResponse {
                success: true,
                response: Some(response),
                session_id: Some(session_id),
                error: None,
            },
            Err(e) => GatewayResponse {
                success: false,
                response: None,
                session_id: Some(session_id),
                error: Some(e.to_string()),
            },
        }
    }

    async fn send_typing(&self, _chat_id: &str) -> AgentResult<()> {
        Ok(()) // Platform-specific implementation
    }
}
