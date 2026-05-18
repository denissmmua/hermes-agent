use tokio::io::AsyncReadExt;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tracing::{error, info};

use hermes_core::AgentResult;

pub type WebhookHandler = Arc<dyn Fn(serde_json::Value) -> AgentResult<serde_json::Value> + Send + Sync>;

pub struct WebhookServer {
    port: u16,
    host: String,
    routes: Arc<Mutex<HashMap<String, WebhookHandler>>>,
}

impl WebhookServer {
    pub fn new(port: u16, host: impl Into<String>) -> Self {
        Self {
            port,
            host: host.into(),
            routes: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn register(&self, path: &str, handler: WebhookHandler) {
        let mut routes = self.routes.lock().await;
        routes.insert(path.to_string(), handler);
    }

    pub async fn start(&self) -> AgentResult<()> {
        let addr = format!("{}:{}", self.host, self.port);
        let listener = TcpListener::bind(&addr).await
            .map_err(|e| hermes_core::AgentError::Config(format!("Bind error: {}", e)))?;

        info!("Webhook server listening on {}", addr);
        let routes = self.routes.clone();

        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((mut socket, _)) => {
                        let routes = routes.clone();
                        tokio::spawn(async move {
                            use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

                            let (reader, mut writer) = socket.split();
                            let mut buf_reader = BufReader::new(reader);
                            let mut request_line = String::new();
                            if buf_reader.read_line(&mut request_line).await.is_err() {
                                return;
                            }

                            // Read headers
                            let mut headers = Vec::new();
                            loop {
                                let mut line = String::new();
                                if buf_reader.read_line(&mut line).await.ok() != Some(0) {
                                    if line.trim().is_empty() {
                                        break;
                                    }
                                    headers.push(line.trim().to_string());
                                }
                            }

                            // Read body if Content-Length present
                            let mut body = String::new();
                            if let Some(cl) = headers.iter().find(|h| h.to_lowercase().starts_with("content-length:")) {
                                if let Some(len_str) = cl.split(':').nth(1) {
                                    if let Ok(len) = len_str.trim().parse::<usize>() {
                                        let mut buf = vec![0u8; len.min(65536)];
                                        if let Ok(n) = buf_reader.read(&mut buf).await {
                                            body = String::from_utf8_lossy(&buf[..n]).to_string();
                                        }
                                    }
                                }
                            }

                            let response = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}";
                            let _ = writer.write_all(response.as_bytes()).await;
                        });
                    }
                    Err(e) => error!("Accept error: {}", e),
                }
            }
        });

        Ok(())
    }
}
