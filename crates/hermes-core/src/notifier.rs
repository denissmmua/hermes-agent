use async_trait::async_trait;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum NotificationLevel { Info, Warning, Error, Success }

pub struct Notification {
    pub level: NotificationLevel,
    pub title: String,
    pub message: String,
    pub source: String,
}

#[async_trait]
pub trait Notifier: Send + Sync {
    fn name(&self) -> &str;
    async fn send(&self, notification: &Notification) -> Result<(), String>;
    fn is_enabled(&self) -> bool;
    fn enable(&mut self);
    fn disable(&mut self);
}

pub struct NotifierManager {
    notifiers: HashMap<String, Box<dyn Notifier>>,
    enabled: bool,
}

impl NotifierManager {
    pub fn new() -> Self { Self { notifiers: HashMap::new(), enabled: true } }

    pub fn register(&mut self, notifier: Box<dyn Notifier>) {
        let name = notifier.name().to_string();
        self.notifiers.insert(name, notifier);
    }

    pub async fn notify(&self, notification: &Notification) {
        if !self.enabled { return; }
        for (_, n) in &self.notifiers {
            if n.is_enabled() {
                let _ = n.send(notification).await;
            }
        }
    }

    pub fn enable(&mut self) { self.enabled = true; }
    pub fn disable(&mut self) { self.enabled = false; }
}

pub struct ConsoleNotifier { enabled: bool }

impl ConsoleNotifier {
    pub fn new() -> Self { Self { enabled: true } }
}

#[async_trait]
impl Notifier for ConsoleNotifier {
    fn name(&self) -> &str { "console" }
    async fn send(&self, n: &Notification) -> Result<(), String> {
        let level = match n.level {
            NotificationLevel::Info => "INFO",
            NotificationLevel::Warning => "WARN",
            NotificationLevel::Error => "ERROR",
            NotificationLevel::Success => "OK",
        };
        println!("[{}] [{}] {}: {}", level, n.source, n.title, n.message);
        Ok(())
    }
    fn is_enabled(&self) -> bool { self.enabled }
    fn enable(&mut self) { self.enabled = true; }
    fn disable(&mut self) { self.enabled = false; }
}
