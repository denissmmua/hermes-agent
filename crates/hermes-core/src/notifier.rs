use std::sync::Arc;
use tokio::sync::broadcast;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// Notification severity levels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum NotificationLevel {
    Info,
    Warning,
    Error,
    Critical,
}

/// A notification event from any subsystem
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub level: NotificationLevel,
    pub source: String,
    pub title: String,
    pub message: String,
    pub metadata: Option<serde_json::Value>,
}

impl Notification {
    pub fn new(
        level: NotificationLevel,
        source: impl Into<String>,
        title: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            level,
            source: source.into(),
            title: title.into(),
            message: message.into(),
            metadata: None,
        }
    }

    pub fn with_metadata(mut self, meta: serde_json::Value) -> Self {
        self.metadata = Some(meta);
        self
    }

    pub fn info(source: impl Into<String>, title: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(NotificationLevel::Info, source, title, message)
    }

    pub fn warning(source: impl Into<String>, title: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(NotificationLevel::Warning, source, title, message)
    }

    pub fn error(source: impl Into<String>, title: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(NotificationLevel::Error, source, title, message)
    }

    pub fn critical(source: impl Into<String>, title: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(NotificationLevel::Critical, source, title, message)
    }
}

/// Event emitted by the notifier
#[derive(Debug, Clone)]
pub enum NotifierEvent {
    Notification(Notification),
    Shutdown,
}

/// Thread-safe notification bus
#[derive(Clone)]
pub struct Notifier {
    tx: broadcast::Sender<NotifierEvent>,
}

impl Notifier {
    /// Create a new notifier with a bounded channel capacity
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self { tx }
    }

    /// Publish a notification to all subscribers
    pub fn notify(&self, notification: Notification) {
        let _ = self.tx.send(NotifierEvent::Notification(notification));
    }

    /// Subscribe to all notifications
    pub fn subscribe(&self) -> broadcast::Receiver<NotifierEvent> {
        self.tx.subscribe()
    }

    /// Number of active subscribers
    pub fn subscriber_count(&self) -> usize {
        self.tx.receiver_count()
    }
}

impl Default for Notifier {
    fn default() -> Self {
        Self::new(256)
    }
}

/// A trait for components that can receive notifications
#[async_trait::async_trait]
pub trait NotificationHandler: Send + Sync {
    async fn handle(&self, notification: &Notification);
}

/// A registry of notification handlers
pub struct NotificationRegistry {
    handlers: Vec<Arc<dyn NotificationHandler>>,
}

impl NotificationRegistry {
    pub fn new() -> Self {
        Self { handlers: Vec::new() }
    }

    pub fn register(&mut self, handler: Arc<dyn NotificationHandler>) {
        self.handlers.push(handler);
    }

    pub async fn dispatch(&self, notification: &Notification) {
        for handler in &self.handlers {
            handler.handle(notification).await;
        }
    }

    pub fn len(&self) -> usize {
        self.handlers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.handlers.is_empty()
    }
}

impl Default for NotificationRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notification_creation() {
        let n = Notification::info("test", "Title", "Message");
        assert_eq!(n.level, NotificationLevel::Info);
        assert_eq!(n.source, "test");
        assert_eq!(n.title, "Title");
        assert_eq!(n.message, "Message");
    }

    #[test]
    fn test_notifier_pub_sub() {
        let notifier = Notifier::new(16);
        let mut rx = notifier.subscribe();

        let n = Notification::warning("test", "Warn", "Something");
        notifier.notify(n.clone());

        // Should receive the notification
        match rx.try_recv() {
            Ok(NotifierEvent::Notification(received)) => {
                assert_eq!(received.title, "Warn");
            }
            _ => panic!("Expected notification"),
        }
    }

    #[test]
    fn test_notification_level_order() {
        assert!(NotificationLevel::Info != NotificationLevel::Error);
        assert_eq!(NotificationLevel::Critical, NotificationLevel::Critical);
    }
}
