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

/// Sound played when a notification is delivered.
///
/// Backends that have no sound surface fall back to [`Silent`](Self::Silent)
/// behaviour and trace the unsupported request (honest partial mapping).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum NotificationSound {
    /// No sound.
    Silent,
    /// The platform's default notification sound.
    #[default]
    Default,
    /// A named system or bundled sound (e.g. `"Ping"`, `"Glass"`).
    Named(String),
    /// The critical-alert sound — louder, and on macOS pierces some
    /// Focus/Do-Not-Disturb states. Backends without a critical sound
    /// fall back to [`Default`](Self::Default).
    Critical,
}

/// What activating a [`NotificationAction`] does.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ActionKind {
    /// A plain button — dismisses the notification and reports its id.
    #[default]
    Button,
    /// A button that brings the delivering app to the foreground.
    Foreground,
    /// A destructive button (rendered emphasised / red on macOS).
    Destructive,
    /// A text-input (reply) action. Carries the send-button title and
    /// the input placeholder.
    TextInput {
        /// Label on the reply's send button.
        button_title: String,
        /// Greyed placeholder shown in the empty reply field.
        placeholder: String,
    },
}

/// One actionable control on a notification — a button or a reply field.
///
/// The `id` is reported back verbatim when the operator activates the
/// action, so a consumer can route it (focus a pane, copy output, open a
/// link, inject a reply). Routing is backend-dependent; a backend that
/// cannot deliver interactive actions reports [`Capabilities::actions`]
/// `= false` and traces the request rather than dropping it silently.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotificationAction {
    /// Stable identifier reported back on activation.
    pub id: String,
    /// Human-visible button label.
    pub title: String,
    /// What activating the action does.
    pub kind: ActionKind,
}

impl NotificationAction {
    /// A plain button.
    #[must_use]
    pub fn button(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self { id: id.into(), title: title.into(), kind: ActionKind::Button }
    }

    /// A button that foregrounds the app.
    #[must_use]
    pub fn foreground(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self { id: id.into(), title: title.into(), kind: ActionKind::Foreground }
    }

    /// A destructive button.
    #[must_use]
    pub fn destructive(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self { id: id.into(), title: title.into(), kind: ActionKind::Destructive }
    }

    /// A text-input (reply) action.
    #[must_use]
    pub fn reply(
        id: impl Into<String>,
        title: impl Into<String>,
        button_title: impl Into<String>,
        placeholder: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            kind: ActionKind::TextInput {
                button_title: button_title.into(),
                placeholder: placeholder.into(),
            },
        }
    }
}

/// Media kind of a [`NotificationAttachment`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AttachmentKind {
    /// An image thumbnail/preview.
    #[default]
    Image,
    /// An audio clip.
    Audio,
    /// A video clip.
    Video,
}

/// A local-file media attachment shown alongside the notification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotificationAttachment {
    /// Local filesystem path to the media.
    pub path: std::path::PathBuf,
    /// Media kind.
    pub kind: AttachmentKind,
}

impl NotificationAttachment {
    /// An image attachment at `path`.
    #[must_use]
    pub fn image(path: impl Into<std::path::PathBuf>) -> Self {
        Self { path: path.into(), kind: AttachmentKind::Image }
    }
}

/// What rich features a backend can actually deliver.
///
/// A backend reports this so consumers degrade *honestly* — an
/// unsupported axis is traced, never silently dropped. The
/// [`LogBackend`](crate::LogBackend) reports [`NONE`](Self::NONE); a full
/// native backend reports [`ALL`](Self::ALL).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capabilities {
    /// Interactive action buttons are delivered.
    pub actions: bool,
    /// Per-notification sound selection is honoured.
    pub sound: bool,
    /// Media attachments are shown.
    pub attachments: bool,
    /// Urgency maps to a real interruption/priority level.
    pub interruption_levels: bool,
    /// Text-input (reply) actions are delivered.
    pub reply: bool,
    /// Re-sending with the same [`id`](Notification::id) updates in place.
    pub update_in_place: bool,
}

impl Capabilities {
    /// No rich features — the honest floor for a log/fallback backend.
    pub const NONE: Self = Self {
        actions: false,
        sound: false,
        attachments: false,
        interruption_levels: false,
        reply: false,
        update_in_place: false,
    };
    /// Every rich feature — a full native backend.
    pub const ALL: Self = Self {
        actions: true,
        sound: true,
        attachments: true,
        interruption_levels: true,
        reply: true,
        update_in_place: true,
    };
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
    /// Optional grouping identifier for collapsing related notifications
    /// (the *thread* identifier on macOS).
    pub group: Option<String>,
    /// Sound played on delivery.
    #[serde(default)]
    pub sound: NotificationSound,
    /// Interactive controls (buttons / reply). Empty for a plain banner.
    #[serde(default)]
    pub actions: Vec<NotificationAction>,
    /// Category identifier — selects a pre-registered action set on
    /// macOS. Independent of [`group`](Self::group) (which threads).
    #[serde(default)]
    pub category: Option<String>,
    /// Optional media attachment (image/audio/video preview).
    #[serde(default)]
    pub attachment: Option<NotificationAttachment>,
    /// Stable identifier. Re-sending with the same id **updates the
    /// existing notification in place** on backends that support it
    /// (see [`Capabilities::update_in_place`]); otherwise a fresh one is
    /// posted. `None` → a random id per send.
    #[serde(default)]
    pub id: Option<String>,
    /// Best-effort auto-withdraw after this long. `None` → sticky.
    #[serde(default)]
    pub timeout: Option<std::time::Duration>,
    /// Custom icon (local file path). Backend-dependent.
    #[serde(default)]
    pub icon: Option<std::path::PathBuf>,
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
            sound: NotificationSound::Default,
            actions: Vec::new(),
            category: None,
            attachment: None,
            id: None,
            timeout: None,
            icon: None,
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

    /// Set the delivery sound.
    #[must_use]
    pub fn sound(mut self, sound: NotificationSound) -> Self {
        self.sound = sound;
        self
    }

    /// Append one interactive action (button / reply).
    #[must_use]
    pub fn action(mut self, action: NotificationAction) -> Self {
        self.actions.push(action);
        self
    }

    /// Replace all interactive actions.
    #[must_use]
    pub fn actions(mut self, actions: Vec<NotificationAction>) -> Self {
        self.actions = actions;
        self
    }

    /// Set the category identifier (selects a pre-registered action set).
    #[must_use]
    pub fn category(mut self, category: impl Into<String>) -> Self {
        self.category = Some(category.into());
        self
    }

    /// Attach a media preview.
    #[must_use]
    pub fn attachment(mut self, attachment: NotificationAttachment) -> Self {
        self.attachment = Some(attachment);
        self
    }

    /// Set the stable identifier (re-send with the same id updates in
    /// place on capable backends).
    #[must_use]
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Set a best-effort auto-withdraw timeout.
    #[must_use]
    pub fn timeout(mut self, timeout: std::time::Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Set a custom icon (local file path).
    #[must_use]
    pub fn icon(mut self, icon: impl Into<std::path::PathBuf>) -> Self {
        self.icon = Some(icon.into());
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

    // ── rich vocabulary (sound / actions / attachment / id / …) ──────

    #[test]
    fn rich_defaults() {
        let n = Notification::new("T", "B");
        assert_eq!(n.sound, NotificationSound::Default);
        assert!(n.actions.is_empty());
        assert_eq!(n.category, None);
        assert_eq!(n.attachment, None);
        assert_eq!(n.id, None);
        assert_eq!(n.timeout, None);
        assert_eq!(n.icon, None);
    }

    #[test]
    fn rich_builder_full_chain() {
        let n = Notification::new("Build", "Done")
            .urgency(Urgency::Critical)
            .sound(NotificationSound::Critical)
            .action(NotificationAction::foreground("focus", "Focus"))
            .action(NotificationAction::reply("reply", "Reply", "Send", "Type…"))
            .category("command-done")
            .attachment(NotificationAttachment::image("/tmp/shot.png"))
            .id("build-42")
            .timeout(std::time::Duration::from_secs(30))
            .icon("/tmp/icon.png");
        assert_eq!(n.sound, NotificationSound::Critical);
        assert_eq!(n.actions.len(), 2);
        assert_eq!(n.actions[0].kind, ActionKind::Foreground);
        assert!(matches!(n.actions[1].kind, ActionKind::TextInput { .. }));
        assert_eq!(n.category.as_deref(), Some("command-done"));
        assert_eq!(n.attachment.as_ref().unwrap().kind, AttachmentKind::Image);
        assert_eq!(n.id.as_deref(), Some("build-42"));
        assert_eq!(n.timeout, Some(std::time::Duration::from_secs(30)));
    }

    #[test]
    fn actions_replaces_whereas_action_appends() {
        let appended = Notification::new("T", "B")
            .action(NotificationAction::button("a", "A"))
            .action(NotificationAction::button("b", "B"));
        assert_eq!(appended.actions.len(), 2);

        let replaced = Notification::new("T", "B")
            .action(NotificationAction::button("a", "A"))
            .actions(vec![NotificationAction::button("z", "Z")]);
        assert_eq!(replaced.actions.len(), 1);
        assert_eq!(replaced.actions[0].id, "z");
    }

    /// The load-bearing back-compat guarantee: JSON written by an *older*
    /// tsuuchi (only the five original fields) must still deserialize —
    /// every new field is `#[serde(default)]`. This keeps every existing
    /// on-wire consumer working after the vocabulary extension.
    #[test]
    fn deserializes_legacy_five_field_json() {
        let legacy = r#"{
            "title": "Old",
            "body": "Client",
            "subtitle": "Sub",
            "urgency": "Critical",
            "group": "g"
        }"#;
        let n: Notification = serde_json::from_str(legacy).unwrap();
        assert_eq!(n.title, "Old");
        assert_eq!(n.urgency, Urgency::Critical);
        assert_eq!(n.group.as_deref(), Some("g"));
        // New fields default cleanly.
        assert_eq!(n.sound, NotificationSound::Default);
        assert!(n.actions.is_empty());
        assert_eq!(n.id, None);
    }

    #[test]
    fn rich_notification_roundtrips_through_serde() {
        let n = Notification::new("T", "B")
            .sound(NotificationSound::Named("Ping".into()))
            .action(NotificationAction::destructive("kill", "Kill"))
            .attachment(NotificationAttachment::image("/tmp/x.png"))
            .id("abc")
            .timeout(std::time::Duration::from_millis(1500));
        let json = serde_json::to_string(&n).unwrap();
        let back: Notification = serde_json::from_str(&json).unwrap();
        assert_eq!(n, back);
    }

    #[test]
    fn sound_default_is_default_variant() {
        assert_eq!(NotificationSound::default(), NotificationSound::Default);
    }

    #[test]
    fn action_constructors_set_kind() {
        assert_eq!(NotificationAction::button("a", "A").kind, ActionKind::Button);
        assert_eq!(NotificationAction::foreground("a", "A").kind, ActionKind::Foreground);
        assert_eq!(NotificationAction::destructive("a", "A").kind, ActionKind::Destructive);
        match NotificationAction::reply("a", "A", "Send", "…").kind {
            ActionKind::TextInput { button_title, placeholder } => {
                assert_eq!(button_title, "Send");
                assert_eq!(placeholder, "…");
            }
            other => panic!("expected TextInput, got {other:?}"),
        }
    }

    #[test]
    fn capabilities_none_and_all() {
        assert!(!Capabilities::NONE.actions);
        assert!(!Capabilities::NONE.sound);
        assert!(Capabilities::ALL.actions);
        assert!(Capabilities::ALL.reply);
        assert!(Capabilities::ALL.update_in_place);
    }
}
