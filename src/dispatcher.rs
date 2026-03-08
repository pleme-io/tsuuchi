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
}
