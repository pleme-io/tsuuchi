//! Notification backend trait and built-in implementations.
//!
//! The [`NotificationBackend`] trait defines the interface that platform
//! backends must implement. A [`LogBackend`] is provided as the default
//! fallback that logs notifications via `tracing`.

use crate::notification::Notification;

use thiserror::Error;

/// Errors that can occur when sending a notification.
#[derive(Debug, Error)]
pub enum TsuuchiError {
    /// The backend failed to deliver the notification.
    #[error("notification send failed: {0}")]
    SendFailed(String),

    /// The backend is not available on this platform.
    #[error("backend unavailable: {0}")]
    Unavailable(String),
}

/// Trait for notification delivery backends.
///
/// Implement this trait to add support for a new platform (e.g., macOS
/// `NSUserNotification`, Linux `libnotify`, or a custom webhook).
pub trait NotificationBackend: Send + Sync {
    /// Send a notification through this backend.
    ///
    /// # Errors
    ///
    /// Returns `TsuuchiError` if the notification could not be delivered.
    fn send(&self, notification: &Notification) -> Result<(), TsuuchiError>;
}

/// A backend that logs notifications via `tracing` instead of delivering
/// them to the OS. Useful as a fallback, for testing, and for headless
/// environments.
#[derive(Debug, Default)]
pub struct LogBackend;

impl LogBackend {
    /// Create a new `LogBackend`.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl NotificationBackend for LogBackend {
    fn send(&self, notification: &Notification) -> Result<(), TsuuchiError> {
        tracing::info!(
            title = %notification.title,
            body = %notification.body,
            subtitle = ?notification.subtitle,
            urgency = ?notification.urgency,
            group = ?notification.group,
            "notification sent (log backend)"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notification::Urgency;

    #[test]
    fn log_backend_succeeds() {
        let backend = LogBackend::new();
        let n = Notification::new("Test", "Body").urgency(Urgency::Low);
        assert!(backend.send(&n).is_ok());
    }

    #[test]
    fn log_backend_with_full_notification() {
        let backend = LogBackend::new();
        let n = Notification::new("Alert", "Something happened")
            .subtitle("Category")
            .urgency(Urgency::Critical)
            .group("alerts");
        assert!(backend.send(&n).is_ok());
    }

    #[test]
    fn log_backend_is_default() {
        let backend = LogBackend::default();
        let n = Notification::new("Default", "Test");
        assert!(backend.send(&n).is_ok());
    }
}
