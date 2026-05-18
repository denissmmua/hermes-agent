use std::collections::HashMap;
use std::sync::Arc;

/// Hook types that can be registered
pub type HookFn = Arc<dyn Fn(&str) -> Option<String> + Send + Sync>;
pub type AsyncHookFn = Arc<dyn Fn(String) -> Box<dyn std::future::Future<Output = String> + Send> + Send + Sync>;

/// Pre/post processing hooks
#[derive(Default)]
pub struct HookSystem {
    pre_message: Vec<(String, HookFn)>,
    post_message: Vec<(String, HookFn)>,
    pre_tool: Vec<(String, HookFn)>,
    post_tool: Vec<(String, HookFn)>,
}

impl HookSystem {
    pub fn new() -> Self { Self::default() }

    /// Register a pre-message hook
    pub fn on_pre_message(&mut self, name: &str, hook: HookFn) {
        self.pre_message.push((name.to_string(), hook));
    }

    /// Register a post-message hook
    pub fn on_post_message(&mut self, name: &str, hook: HookFn) {
        self.post_message.push((name.to_string(), hook));
    }

    /// Run all pre-message hooks
    pub fn run_pre_message(&self, msg: &str) -> String {
        let mut result = msg.to_string();
        for (name, hook) in &self.pre_message {
            if let Some(modified) = hook(&result) {
                result = modified;
            }
        }
        result
    }

    /// Run all post-message hooks
    pub fn run_post_message(&self, msg: &str) -> String {
        let mut result = msg.to_string();
        for (name, hook) in &self.post_message {
            if let Some(modified) = hook(&result) {
                result = modified;
            }
        }
        result
    }
}
