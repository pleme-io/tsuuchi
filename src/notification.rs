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

    #[test]
    fn builder_accepts_string_types() {
        let owned_title = String::from("Owned");
        let owned_body = String::from("Body");
        let n = Notification::new(owned_title, owned_body)
            .subtitle(String::from("Sub"))
            .group(String::from("grp"));
        assert_eq!(n.title, "Owned");
        assert_eq!(n.body, "Body");
        assert_eq!(n.subtitle.as_deref(), Some("Sub"));
        assert_eq!(n.group.as_deref(), Some("grp"));
    }

    #[test]
    fn builder_only_subtitle_set() {
        let n = Notification::new("T", "B").subtitle("S");
        assert_eq!(n.subtitle.as_deref(), Some("S"));
        assert_eq!(n.urgency, Urgency::Normal);
        assert_eq!(n.group, None);
    }

    #[test]
    fn builder_only_group_set() {
        let n = Notification::new("T", "B").group("G");
        assert_eq!(n.group.as_deref(), Some("G"));
        assert_eq!(n.subtitle, None);
        assert_eq!(n.urgency, Urgency::Normal);
    }

    #[test]
    fn builder_only_urgency_set() {
        let n = Notification::new("T", "B").urgency(Urgency::Low);
        assert_eq!(n.urgency, Urgency::Low);
        assert_eq!(n.subtitle, None);
        assert_eq!(n.group, None);
    }

    #[test]
    fn builder_chaining_order_does_not_matter() {
        let n1 = Notification::new("T", "B")
            .urgency(Urgency::Critical)
            .subtitle("S")
            .group("G");

        let n2 = Notification::new("T", "B")
            .group("G")
            .urgency(Urgency::Critical)
            .subtitle("S");

        assert_eq!(n1, n2);
    }

    #[test]
    fn urgency_last_set_wins() {
        let n = Notification::new("T", "B")
            .urgency(Urgency::Low)
            .urgency(Urgency::Critical);
        assert_eq!(n.urgency, Urgency::Critical);
    }

    #[test]
    fn subtitle_last_set_wins() {
        let n = Notification::new("T", "B")
            .subtitle("First")
            .subtitle("Second");
        assert_eq!(n.subtitle.as_deref(), Some("Second"));
    }

    #[test]
    fn group_last_set_wins() {
        let n = Notification::new("T", "B")
            .group("alpha")
            .group("beta");
        assert_eq!(n.group.as_deref(), Some("beta"));
    }

    #[test]
    fn empty_title_and_body_are_valid() {
        let n = Notification::new("", "");
        assert_eq!(n.title, "");
        assert_eq!(n.body, "");
    }

    #[test]
    fn unicode_content() {
        let n = Notification::new("通知タイトル", "本文テスト 🔔")
            .subtitle("サブ")
            .group("グループ");
        assert_eq!(n.title, "通知タイトル");
        assert_eq!(n.body, "本文テスト 🔔");
        assert_eq!(n.subtitle.as_deref(), Some("サブ"));
        assert_eq!(n.group.as_deref(), Some("グループ"));
    }

    #[test]
    fn notification_clone_is_independent() {
        let n1 = Notification::new("Title", "Body")
            .subtitle("Sub")
            .urgency(Urgency::Critical)
            .group("grp");
        let n2 = n1.clone();

        assert_eq!(n1, n2);
        // Verify they are structurally equal but separate allocations.
        assert_eq!(n2.title, "Title");
        assert_eq!(n2.subtitle.as_deref(), Some("Sub"));
    }

    #[test]
    fn notification_debug_format() {
        let n = Notification::new("T", "B");
        let debug = format!("{n:?}");
        assert!(debug.contains("Notification"));
        assert!(debug.contains("T"));
        assert!(debug.contains("B"));
    }

    #[test]
    fn urgency_clone_and_copy() {
        let u = Urgency::Critical;
        let u2 = u; // Copy
        let u3 = u.clone();
        assert_eq!(u, u2);
        assert_eq!(u, u3);
    }

    #[test]
    fn urgency_hash_consistency() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(Urgency::Low);
        set.insert(Urgency::Normal);
        set.insert(Urgency::Critical);
        assert_eq!(set.len(), 3);

        // Duplicate insert should not increase length.
        set.insert(Urgency::Low);
        assert_eq!(set.len(), 3);
    }

    #[test]
    fn urgency_all_variants_ordered() {
        let mut urgencies = [Urgency::Critical, Urgency::Low, Urgency::Normal];
        urgencies.sort();
        assert_eq!(urgencies, [Urgency::Low, Urgency::Normal, Urgency::Critical]);
    }

    #[test]
    fn serialization_minimal_notification() {
        let n = Notification::new("T", "B");
        let json = serde_json::to_value(&n).unwrap();
        assert_eq!(json["title"], "T");
        assert_eq!(json["body"], "B");
        assert!(json["subtitle"].is_null());
        assert!(json["group"].is_null());
        assert_eq!(json["urgency"], "Normal");
    }

    #[test]
    fn deserialization_from_json_string() {
        let json = r#"{
            "title": "Deserialized",
            "body": "From JSON",
            "subtitle": "Sub",
            "urgency": "Critical",
            "group": "test"
        }"#;
        let n: Notification = serde_json::from_str(json).unwrap();
        assert_eq!(n.title, "Deserialized");
        assert_eq!(n.body, "From JSON");
        assert_eq!(n.subtitle.as_deref(), Some("Sub"));
        assert_eq!(n.urgency, Urgency::Critical);
        assert_eq!(n.group.as_deref(), Some("test"));
    }

    #[test]
    fn deserialization_with_null_optionals() {
        let json = r#"{
            "title": "T",
            "body": "B",
            "subtitle": null,
            "urgency": "Low",
            "group": null
        }"#;
        let n: Notification = serde_json::from_str(json).unwrap();
        assert_eq!(n.subtitle, None);
        assert_eq!(n.group, None);
        assert_eq!(n.urgency, Urgency::Low);
    }

    #[test]
    fn urgency_serialization_roundtrip() {
        for urgency in [Urgency::Low, Urgency::Normal, Urgency::Critical] {
            let json = serde_json::to_string(&urgency).unwrap();
            let deserialized: Urgency = serde_json::from_str(&json).unwrap();
            assert_eq!(urgency, deserialized);
        }
    }

    #[test]
    fn notification_equality() {
        let n1 = Notification::new("A", "B").urgency(Urgency::Low);
        let n2 = Notification::new("A", "B").urgency(Urgency::Low);
        let n3 = Notification::new("A", "B").urgency(Urgency::Critical);

        assert_eq!(n1, n2);
        assert_ne!(n1, n3);
    }

    #[test]
    fn notification_inequality_on_each_field() {
        let base = Notification::new("T", "B")
            .subtitle("S")
            .urgency(Urgency::Normal)
            .group("G");

        // Different title.
        assert_ne!(base, Notification::new("X", "B").subtitle("S").urgency(Urgency::Normal).group("G"));
        // Different body.
        assert_ne!(base, Notification::new("T", "X").subtitle("S").urgency(Urgency::Normal).group("G"));
        // Different subtitle.
        assert_ne!(base, Notification::new("T", "B").subtitle("X").urgency(Urgency::Normal).group("G"));
        // Different urgency.
        assert_ne!(base, Notification::new("T", "B").subtitle("S").urgency(Urgency::Critical).group("G"));
        // Different group.
        assert_ne!(base, Notification::new("T", "B").subtitle("S").urgency(Urgency::Normal).group("X"));
        // Missing subtitle vs present.
        assert_ne!(base, Notification::new("T", "B").urgency(Urgency::Normal).group("G"));
        // Missing group vs present.
        assert_ne!(base, Notification::new("T", "B").subtitle("S").urgency(Urgency::Normal));
    }
}
