//! Notification data model with builder pattern.
//!
//! Provides the core [`Notification`] struct and [`Urgency`] enum used
//! throughout the notification pipeline.

use serde::{Deserialize, Serialize};

/// Urgency level for a notification.
///
/// Ordered from least to most urgent. Backends may use this to decide
/// presentation style (e.g., silent vs. alert sound, banner vs. modal).
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub enum Urgency {
    /// Informational, non-intrusive.
    Low,
    /// Standard notification.
    #[default]
    Normal,
    /// Requires immediate attention.
    Critical,
}

/// A notification to be sent through a backend.
///
/// Use the builder pattern via [`Notification::new`] to construct:
///
/// ```
/// use tsuuchi::{Notification, Urgency};
///
/// let n = Notification::new("Build Complete", "All 42 tests passed")
///     .subtitle("CI Pipeline")
///     .urgency(Urgency::Low)
///     .group("ci");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Notification {
    /// The notification title (primary line).
    pub title: String,
    /// The notification body (secondary text).
    pub body: String,
    /// Optional subtitle (shown between title and body on macOS).
    pub subtitle: Option<String>,
    /// Urgency level.
    pub urgency: Urgency,
    /// Optional grouping identifier for collapsing related notifications.
    pub group: Option<String>,
}

impl Notification {
    /// Create a new notification with the given title and body.
    ///
    /// Defaults to [`Urgency::Normal`] with no subtitle or group.
    #[must_use]
    pub fn new(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            body: body.into(),
            subtitle: None,
            urgency: Urgency::Normal,
            group: None,
        }
    }

    /// Set the subtitle.
    #[must_use]
    pub fn subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// Set the urgency level.
    #[must_use]
    pub fn urgency(mut self, urgency: Urgency) -> Self {
        self.urgency = urgency;
        self
    }

    /// Set the group identifier for notification collapsing.
    #[must_use]
    pub fn group(mut self, group: impl Into<String>) -> Self {
        self.group = Some(group.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_defaults() {
        let n = Notification::new("Title", "Body");
        assert_eq!(n.title, "Title");
        assert_eq!(n.body, "Body");
        assert_eq!(n.subtitle, None);
        assert_eq!(n.urgency, Urgency::Normal);
        assert_eq!(n.group, None);
    }

    #[test]
    fn builder_full_chain() {
        let n = Notification::new("Alert", "Disk full")
            .subtitle("System")
            .urgency(Urgency::Critical)
            .group("disk");

        assert_eq!(n.title, "Alert");
        assert_eq!(n.body, "Disk full");
        assert_eq!(n.subtitle.as_deref(), Some("System"));
        assert_eq!(n.urgency, Urgency::Critical);
        assert_eq!(n.group.as_deref(), Some("disk"));
    }

    #[test]
    fn urgency_ordering() {
        assert!(Urgency::Low < Urgency::Normal);
        assert!(Urgency::Normal < Urgency::Critical);
        assert!(Urgency::Low < Urgency::Critical);
    }

    #[test]
    fn urgency_default_is_normal() {
        assert_eq!(Urgency::default(), Urgency::Normal);
    }

    #[test]
    fn notification_is_serializable() {
        let n = Notification::new("Test", "Body")
            .subtitle("Sub")
            .urgency(Urgency::Critical)
            .group("g");

        let json = serde_json::to_string(&n).unwrap();
        let deserialized: Notification = serde_json::from_str(&json).unwrap();
        assert_eq!(n, deserialized);
    }
}
