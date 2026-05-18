use tokio::sync::mpsc;
use hermes_core::AgentResult;
use hermes_gateway::{LLMMessage, LLMProvider, LLMRequest};

pub struct StreamingTurn {
    pub delta_rx: mpsc::UnboundedReceiver<String>,
}

impl StreamingTurn {
    pub async fn collect(&mut self) -> String {
        let mut full = String::new();
        while let Some(delta) = self.delta_rx.recv().await {
            full.push_str(&delta);
        }
        full
    }
}

pub async fn stream_turn(
    gateway: std::sync::Arc<dyn LLMProvider>,
    model: String,
    messages: Vec<LLMMessage>,
    temperature: f64,
) -> AgentResult<StreamingTurn> {
    let (tx, rx) = mpsc::unbounded_channel();
    let error_tx = tx.clone();

    tokio::spawn(async move {
        let request = LLMRequest {
            model,
            messages,
            max_tokens: Some(4096),
            temperature: Some(temperature),
            stop: None,
            stream: true,
            tools: Vec::new(),
        };
        let callback_tx = tx.clone();
        match gateway.chat_stream(request, Box::new(move |delta| {
            let _ = callback_tx.send(delta);
        })).await {
            Ok(_) => {}
            Err(e) => {
                let _ = error_tx.send(format!("\n[Error: {}]", e));
            }
        }
    });

    Ok(StreamingTurn {
        delta_rx: rx,
    })
}
