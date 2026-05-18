use std::sync::Arc;
use hermes_core::AgentResult;
use crate::platforms::{Platform, PlatformMessage};

/// Mirror messages from one platform to others
pub struct Mirror {
    pub primary: Arc<dyn Platform>,
    pub mirrors: Vec<Arc<dyn Platform>>,
}

impl Mirror {
    pub fn new(primary: Arc<dyn Platform>) -> Self {
        Self { primary, mirrors: Vec::new() }
    }

    pub fn add_mirror(&mut self, platform: Arc<dyn Platform>) {
        self.mirrors.push(platform);
    }

    pub async fn broadcast(&self, msg: &PlatformMessage) {
        for mirror in &self.mirrors {
            let _ = mirror.send_message(&msg.chat_id, &msg.text).await;
        }
    }
}
