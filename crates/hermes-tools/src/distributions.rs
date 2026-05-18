// 1:1 port of toolset_distributions.py - tool distribution rules
use std::collections::HashMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDistribution {
    pub tool_name: String,
    pub priority: u32,
    pub max_calls: u32,
    pub timeout_secs: u64,
    pub allowed_contexts: Vec<String>,
}

pub fn default_distributions() -> HashMap<String, ToolDistribution> {
    let mut d = HashMap::new();
    for (name, priority, max_calls, timeout, contexts) in [
        ("shell", 10u32, 50u32, 120u64, vec!["coding", "admin", "debug"]),
        ("read_file", 20, 100, 30, vec!["coding", "research"]),
        ("write_file", 15, 50, 30, vec!["coding"]),
        ("grep", 25, 100, 30, vec!["coding", "research"]),
        ("git", 20, 30, 30, vec!["coding"]),
        ("web_fetch", 10, 20, 15, vec!["research"]),
    ] {
        d.insert(name.to_string(), ToolDistribution {
            tool_name: name.to_string(), priority, max_calls, timeout_secs: timeout,
            allowed_contexts: contexts.iter().map(|s| s.to_string()).collect(),
        });
    }
    d
}

pub struct ToolDispatch {
    distributions: HashMap<String, ToolDistribution>,
    call_counts: HashMap<String, u32>,
}

impl ToolDispatch {
    pub fn new() -> Self {
        Self { distributions: default_distributions(), call_counts: HashMap::new() }
    }

    pub fn can_call(&mut self, name: &str) -> bool {
        let count = self.call_counts.get(name).copied().unwrap_or(0);
        let max = self.distributions.get(name).map(|d| d.max_calls).unwrap_or(50);
        if count >= max { return false; }
        self.call_counts.insert(name.to_string(), count + 1);
        true
    }

    pub fn reset(&mut self) {
        self.call_counts.clear();
    }

    pub fn timeout_for(&self, name: &str) -> u64 {
        self.distributions.get(name).map(|d| d.timeout_secs).unwrap_or(30)
    }
}
