#![allow(dead_code)]
/// Message delivery — routes responses to the right platform
use std::sync::Arc;
use hermes_core::AgentResult;
use crate::Platform;

pub struct MessageRoute {
    pub platform: Arc<dyn Platform>,
    pub chat_id: String,
    pub thread_id: Option<String>,
}

pub struct DeliveryManager {
    pub routes: Vec<MessageRoute>,
}

impl DeliveryManager {
    pub fn new() -> Self { Self { routes: Vec::new() } }

    pub fn add_route(&mut self, platform: Arc<dyn Platform>, chat_id: &str) {
        self.routes.push(MessageRoute {
            platform,
            chat_id: chat_id.to_string(),
            thread_id: None,
        });
    }

    pub async fn deliver(&self, text: &str) -> AgentResult<()> {
        for route in &self.routes {
            route.platform.send_message(&route.chat_id, text).await?;
        }
        Ok(())
    }
}
