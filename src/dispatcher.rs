//! Notification dispatcher that routes notifications through a backend.
//!
//! The [`NotificationDispatcher`] holds a boxed [`NotificationBackend`]
//! and provides a simple `send()` method for delivering notifications.

use crate::backend::{NotificationBackend, TsuuchiError};
use crate::notification::Notification;

/// Routes notifications through a configured backend.
///
/// Owns a `Box<dyn NotificationBackend>` and delegates all `send()`
/// calls to it. Swap the backend at construction time to change
/// delivery behavior (e.g., `LogBackend` in tests, `MacOSBackend`
/// in production).
pub struct NotificationDispatcher {
    backend: Box<dyn NotificationBackend>,
}

impl NotificationDispatcher {
    /// Create a dispatcher with the given backend.
    #[must_use]
    pub fn new(backend: Box<dyn NotificationBackend>) -> Self {
        Self { backend }
    }

    /// Send a notification through the configured backend.
    ///
    /// # Errors
    ///
    /// Returns `TsuuchiError` if the backend fails to deliver.
    pub fn send(&self, notification: &Notification) -> Result<(), TsuuchiError> {
        self.backend.send(notification)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::LogBackend;

    #[test]
    fn dispatcher_sends_through_backend() {
        let dispatcher = NotificationDispatcher::new(Box::new(LogBackend::new()));
        let n = Notification::new("Test", "Works");
        assert!(dispatcher.send(&n).is_ok());
    }

    #[test]
    fn dispatcher_with_custom_backend() {
        /// A test backend that always fails.
        struct FailBackend;

        impl NotificationBackend for FailBackend {
            fn send(&self, _notification: &Notification) -> Result<(), TsuuchiError> {
                Err(TsuuchiError::SendFailed("intentional failure".into()))
            }
        }

        let dispatcher = NotificationDispatcher::new(Box::new(FailBackend));
        let n = Notification::new("Test", "Should fail");
        assert!(dispatcher.send(&n).is_err());
    }

    #[test]
    fn dispatcher_propagates_send_failed_error() {
        struct FailBackend;

        impl NotificationBackend for FailBackend {
            fn send(&self, _notification: &Notification) -> Result<(), TsuuchiError> {
                Err(TsuuchiError::SendFailed("connection refused".into()))
            }
        }

        let dispatcher = NotificationDispatcher::new(Box::new(FailBackend));
        let n = Notification::new("T", "B");
        let err = dispatcher.send(&n).unwrap_err();
        assert!(matches!(err, TsuuchiError::SendFailed(_)));
        assert!(err.to_string().contains("connection refused"));
    }

    #[test]
    fn dispatcher_propagates_unavailable_error() {
        struct UnavailableBackend;

        impl NotificationBackend for UnavailableBackend {
            fn send(&self, _notification: &Notification) -> Result<(), TsuuchiError> {
                Err(TsuuchiError::Unavailable("no display".into()))
            }
        }

        let dispatcher = NotificationDispatcher::new(Box::new(UnavailableBackend));
        let n = Notification::new("T", "B");
        let err = dispatcher.send(&n).unwrap_err();
        assert!(matches!(err, TsuuchiError::Unavailable(_)));
        assert!(err.to_string().contains("no display"));
    }

    #[test]
    fn dispatcher_sends_multiple_notifications() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct CountBackend {
            count: AtomicUsize,
        }

        impl NotificationBackend for CountBackend {
            fn send(&self, _notification: &Notification) -> Result<(), TsuuchiError> {
                self.count.fetch_add(1, Ordering::Relaxed);
                Ok(())
            }
        }

        let counter = std::sync::Arc::new(CountBackend {
            count: AtomicUsize::new(0),
        });

        // We need to clone the Arc before moving into Box.
        let backend_ref = std::sync::Arc::clone(&counter);

        // Use a wrapper to get Arc into Box<dyn NotificationBackend>.
        struct ArcWrapper(std::sync::Arc<CountBackend>);

        impl NotificationBackend for ArcWrapper {
            fn send(&self, notification: &Notification) -> Result<(), TsuuchiError> {
                self.0.send(notification)
            }
        }

        let dispatcher = NotificationDispatcher::new(Box::new(ArcWrapper(backend_ref)));

        for i in 0..5 {
            let n = Notification::new(format!("Notif {i}"), "body");
            dispatcher.send(&n).unwrap();
        }

        assert_eq!(counter.count.load(Ordering::Relaxed), 5);
    }

    #[test]
    fn dispatcher_preserves_notification_data_through_backend() {
        use std::sync::Mutex;

        struct RecordBackend {
            recorded: Mutex<Vec<Notification>>,
        }

        impl NotificationBackend for RecordBackend {
            fn send(&self, notification: &Notification) -> Result<(), TsuuchiError> {
                self.recorded.lock().unwrap().push(notification.clone());
                Ok(())
            }
        }

        let backend = std::sync::Arc::new(RecordBackend {
            recorded: Mutex::new(Vec::new()),
        });

        struct ArcWrapper(std::sync::Arc<RecordBackend>);

        impl NotificationBackend for ArcWrapper {
            fn send(&self, notification: &Notification) -> Result<(), TsuuchiError> {
                self.0.send(notification)
            }
        }

        let backend_ref = std::sync::Arc::clone(&backend);
        let dispatcher = NotificationDispatcher::new(Box::new(ArcWrapper(backend_ref)));

        let n = Notification::new("Title", "Body")
            .subtitle("Sub")
            .urgency(crate::notification::Urgency::Critical)
            .group("grp");

        dispatcher.send(&n).unwrap();

        let recorded = backend.recorded.lock().unwrap();
        assert_eq!(recorded.len(), 1);
        assert_eq!(recorded[0], n);
    }

    #[test]
    fn dispatcher_with_log_backend_all_urgencies() {
        let dispatcher = NotificationDispatcher::new(Box::new(LogBackend::new()));

        for urgency in [
            crate::notification::Urgency::Low,
            crate::notification::Urgency::Normal,
            crate::notification::Urgency::Critical,
        ] {
            let n = Notification::new("Test", "Body").urgency(urgency);
            assert!(dispatcher.send(&n).is_ok());
        }
    }
}
