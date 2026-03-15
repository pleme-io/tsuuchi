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

    #[test]
    fn log_backend_handles_empty_notification() {
        let backend = LogBackend::new();
        let n = Notification::new("", "");
        assert!(backend.send(&n).is_ok());
    }

    #[test]
    fn log_backend_handles_unicode() {
        let backend = LogBackend::new();
        let n = Notification::new("通知", "テスト 🔔")
            .subtitle("サブ")
            .group("グループ");
        assert!(backend.send(&n).is_ok());
    }

    #[test]
    fn log_backend_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<LogBackend>();
    }

    #[test]
    fn log_backend_repeated_sends() {
        let backend = LogBackend::new();
        let n = Notification::new("Repeated", "Test");
        for _ in 0..100 {
            assert!(backend.send(&n).is_ok());
        }
    }

    #[test]
    fn tsuuchi_error_send_failed_message() {
        let err = TsuuchiError::SendFailed("disk full".into());
        let msg = err.to_string();
        assert!(msg.contains("disk full"));
        assert!(msg.contains("notification send failed"));
    }

    #[test]
    fn tsuuchi_error_unavailable_message() {
        let err = TsuuchiError::Unavailable("no display server".into());
        let msg = err.to_string();
        assert!(msg.contains("no display server"));
        assert!(msg.contains("backend unavailable"));
    }

    #[test]
    fn tsuuchi_error_is_debug() {
        let err = TsuuchiError::SendFailed("test".into());
        let debug = format!("{err:?}");
        assert!(debug.contains("SendFailed"));
    }

    /// A backend that counts how many times send was called.
    struct CountingBackend {
        count: std::sync::atomic::AtomicUsize,
    }

    impl CountingBackend {
        fn new() -> Self {
            Self {
                count: std::sync::atomic::AtomicUsize::new(0),
            }
        }

        fn count(&self) -> usize {
            self.count.load(std::sync::atomic::Ordering::Relaxed)
        }
    }

    impl NotificationBackend for CountingBackend {
        fn send(&self, _notification: &Notification) -> Result<(), TsuuchiError> {
            self.count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Ok(())
        }
    }

    #[test]
    fn custom_backend_trait_implementation() {
        let backend = CountingBackend::new();
        let n = Notification::new("A", "B");

        assert_eq!(backend.count(), 0);
        backend.send(&n).unwrap();
        backend.send(&n).unwrap();
        backend.send(&n).unwrap();
        assert_eq!(backend.count(), 3);
    }

    /// A backend that captures the last notification sent.
    struct CapturingBackend {
        last: std::sync::Mutex<Option<Notification>>,
    }

    impl CapturingBackend {
        fn new() -> Self {
            Self {
                last: std::sync::Mutex::new(None),
            }
        }

        fn last_notification(&self) -> Option<Notification> {
            self.last.lock().unwrap().clone()
        }
    }

    impl NotificationBackend for CapturingBackend {
        fn send(&self, notification: &Notification) -> Result<(), TsuuchiError> {
            *self.last.lock().unwrap() = Some(notification.clone());
            Ok(())
        }
    }

    #[test]
    fn capturing_backend_stores_notification() {
        let backend = CapturingBackend::new();
        assert!(backend.last_notification().is_none());

        let n = Notification::new("Capture", "Me")
            .subtitle("Sub")
            .urgency(Urgency::Critical)
            .group("cap");

        backend.send(&n).unwrap();

        let captured = backend.last_notification().unwrap();
        assert_eq!(captured.title, "Capture");
        assert_eq!(captured.body, "Me");
        assert_eq!(captured.subtitle.as_deref(), Some("Sub"));
        assert_eq!(captured.urgency, Urgency::Critical);
        assert_eq!(captured.group.as_deref(), Some("cap"));
    }

    /// A backend that fails after N successful sends.
    struct FlakeyBackend {
        remaining: std::sync::atomic::AtomicUsize,
    }

    impl FlakeyBackend {
        fn new(successes: usize) -> Self {
            Self {
                remaining: std::sync::atomic::AtomicUsize::new(successes),
            }
        }
    }

    impl NotificationBackend for FlakeyBackend {
        fn send(&self, _notification: &Notification) -> Result<(), TsuuchiError> {
            if self.remaining.fetch_sub(1, std::sync::atomic::Ordering::Relaxed) > 0 {
                Ok(())
            } else {
                Err(TsuuchiError::SendFailed("flakey backend exhausted".into()))
            }
        }
    }

    #[test]
    fn flakey_backend_succeeds_then_fails() {
        let backend = FlakeyBackend::new(2);
        let n = Notification::new("T", "B");

        assert!(backend.send(&n).is_ok());
        assert!(backend.send(&n).is_ok());
        assert!(backend.send(&n).is_err());
    }

    /// A backend that only accepts Critical notifications.
    struct CriticalOnlyBackend;

    impl NotificationBackend for CriticalOnlyBackend {
        fn send(&self, notification: &Notification) -> Result<(), TsuuchiError> {
            if notification.urgency == Urgency::Critical {
                Ok(())
            } else {
                Err(TsuuchiError::SendFailed(format!(
                    "only Critical accepted, got {:?}",
                    notification.urgency
                )))
            }
        }
    }

    #[test]
    fn backend_can_filter_by_urgency() {
        let backend = CriticalOnlyBackend;
        let critical = Notification::new("T", "B").urgency(Urgency::Critical);
        let low = Notification::new("T", "B").urgency(Urgency::Low);
        let normal = Notification::new("T", "B");

        assert!(backend.send(&critical).is_ok());
        assert!(backend.send(&low).is_err());
        assert!(backend.send(&normal).is_err());
    }

    #[test]
    fn boxed_backend_as_trait_object() {
        let backend: Box<dyn NotificationBackend> = Box::new(LogBackend::new());
        let n = Notification::new("Boxed", "Test");
        assert!(backend.send(&n).is_ok());
    }
}
