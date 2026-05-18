#![allow(dead_code)]
/// Channel directory — routes message types to platforms
use std::collections::HashMap;

/// Channel type
#[derive(Debug, Clone, PartialEq, Hash, Eq)]
pub enum ChannelType {
    Cli,
    Telegram,
    Discord,
    Slack,
    Web,
    Api,
    Email,
    Sms,
    Custom(String),
}

pub struct ChannelDirectory {
    channels: HashMap<ChannelType, String>,
}

impl ChannelDirectory {
    pub fn new() -> Self { Self { channels: HashMap::new() } }

    pub fn register(&mut self, channel_type: ChannelType, endpoint: &str) {
        self.channels.insert(channel_type, endpoint.to_string());
    }

    pub fn get(&self, channel_type: &ChannelType) -> Option<&String> {
        self.channels.get(channel_type)
    }

    pub fn broadcast_channels(&self) -> Vec<&ChannelType> {
        self.channels.keys().collect()
    }
}
