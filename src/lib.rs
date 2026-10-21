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

pub mod action_wire;
pub mod backend;
#[cfg(target_os = "macos")]
pub mod backend_macos;
#[cfg(unix)]
pub mod backend_shirase;
pub mod dispatcher;
pub mod history;
pub mod notification;

pub use action_wire::{ActionMessage, ActionRoute, FOCUS_PANE_ACTION_ID};
pub use backend::{LogBackend, NotificationBackend, TsuuchiError};
#[cfg(target_os = "macos")]
pub use backend_macos::UnBackend;
#[cfg(unix)]
pub use backend_shirase::ShiraseBackend;
pub use dispatcher::NotificationDispatcher;
pub use history::{HistoryEntry, NotificationHistory};
pub use notification::{
    ActionKind, AttachmentKind, Capabilities, Notification, NotificationAction,
    NotificationAttachment, NotificationSound, Urgency,
};
