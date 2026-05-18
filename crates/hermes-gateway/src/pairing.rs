#![allow(dead_code)]
use std::collections::HashMap;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairedDevice {
    pub id: String,
    pub name: String,
    pub platform: String,
    pub user_id: String,
    pub paired_at: String,
    pub last_seen: String,
}

pub struct PairingManager {
    devices: HashMap<String, PairedDevice>,
}

impl PairingManager {
    pub fn new() -> Self { Self { devices: HashMap::new() } }

    pub fn pair(&mut self, id: &str, name: &str, platform: &str, user_id: &str) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            .to_string();
        self.devices.insert(id.to_string(), PairedDevice {
            id: id.to_string(),
            name: name.to_string(),
            platform: platform.to_string(),
            user_id: user_id.to_string(),
            paired_at: now.clone(),
            last_seen: now,
        });
    }

    pub fn unpair(&mut self, id: &str) { self.devices.remove(id); }
    pub fn get(&self, id: &str) -> Option<&PairedDevice> { self.devices.get(id) }
    pub fn all(&self) -> Vec<&PairedDevice> { self.devices.values().collect() }
}
