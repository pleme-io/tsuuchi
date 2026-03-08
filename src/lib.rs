//! Tsuuchi (通知) — platform-agnostic notification framework.
//!
//! Provides a trait-based notification system with pluggable backends.
//! Ships with a `LogBackend` for testing and headless use; platform
//! backends (macOS, Linux) can be added by implementing
//! [`NotificationBackend`].
//!
//! # Quick Start
//!
//! ```
//! use tsuuchi::{Notification, Urgency, NotificationDispatcher, LogBackend};
//!
//! let dispatcher = NotificationDispatcher::new(Box::new(LogBackend::new()));
//!
//! let n = Notification::new("Build Complete", "All tests passed")
//!     .subtitle("CI")
//!     .urgency(Urgency::Low)
//!     .group("ci");
//!
//! dispatcher.send(&n).unwrap();
//! ```

pub mod backend;
pub mod dispatcher;
pub mod history;
pub mod notification;

pub use backend::{LogBackend, NotificationBackend, TsuuchiError};
pub use dispatcher::NotificationDispatcher;
pub use history::{HistoryEntry, NotificationHistory};
pub use notification::{Notification, Urgency};
