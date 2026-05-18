use futures::StreamExt;
use reqwest::Client;
use serde_json::Value;
use tokio::sync::mpsc;

use hermes_core::{AgentError, AgentResult};

pub fn parse_sse_line(line: &str) -> Option<String> {
    if let Some(data) = line.strip_prefix("data: ") {
        if data != "[DONE]" {
            return Some(data.to_string());
        }
    }
    None
}

pub fn extract_delta(chunk: &Value) -> Option<String> {
    chunk["choices"][0]["delta"]["content"]
        .as_str()
        .map(|s| s.to_string())
}

pub async fn stream_chat(
    client: &Client,
    url: &str,
    api_key: &str,
    body: Value,
    tx: mpsc::UnboundedSender<String>,
) -> AgentResult<String> {
    let response = client
        .post(url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .header("Accept", "text/event-stream")
        .json(&body)
        .send()
        .await
        .map_err(|e| AgentError::Provider(format!("Stream HTTP error: {}", e)))?;

    let mut full_content = String::new();
    let mut stream = response.bytes_stream();

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.map_err(|e| AgentError::Provider(format!("Stream error: {}", e)))?;
        let text = String::from_utf8_lossy(&chunk);

        for line in text.lines() {
            if let Some(data) = parse_sse_line(line) {
                if let Ok(val) = serde_json::from_str::<Value>(&data) {
                    if let Some(delta) = extract_delta(&val) {
                        full_content.push_str(&delta);
                        let _ = tx.send(delta);
                    }
                }
            }
        }
    }

    Ok(full_content)
}
