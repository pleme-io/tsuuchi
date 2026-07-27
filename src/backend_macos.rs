//! Native macOS notifications via `UNUserNotificationCenter`.
//!
//! Replaces the legacy `osascript display notification` shell-out — which
//! macOS attributed to *Script Editor* and which tripped the automation
//! permission popup. This backend delivers **app-attributed** banners with
//! sound, an `Urgency → interruption-level` mapping, and thread grouping.
//!
//! It is only viable when the host process runs **bundled** (a `.app` with
//! a `CFBundleIdentifier`): `UNUserNotificationCenter` aborts for a process
//! with no bundle identifier. [`UnBackend::try_new`] returns `None` when
//! unbundled so the caller falls back (log/dock) — never the popup.
//!
//! Threading: `send` builds every Objective-C object locally and never
//! stores a `Retained` across threads, so [`UnBackend`] is a ZST and
//! trivially `Send + Sync`. `UNUserNotificationCenter` is documented safe
//! to message from any thread.
//!
//! # Why this lives in tsuuchi
//!
//! It previously lived in **mado**, a *consumer*. That made the fleet's
//! notification primitive hollow on its primary platform: tsuuchi shipped
//! only `LogBackend`, while the single real `UNUserNotificationCenter`
//! binding sat inside one application — so every other fleet app either
//! went without native notifications or would have re-rolled the same
//! objc2 code. Owning the platform binding here is the solve-once fix; a
//! consumer now writes `NotificationDispatcher::new(Box::new(UnBackend::try_new()?))`.
//!
//! # Tier honesty
//!
//! Delivers content + sound + interruption level + thread + stable-id
//! (update-in-place). Action buttons, attachments and inline reply travel
//! in the tsuuchi vocabulary but are **not yet routed** — they need a
//! `UNUserNotificationCenterDelegate` response path, which requires a
//! retained delegate object and an app run-loop, so it is deliberately not
//! attempted here. [`UnBackend::capabilities`] reports each of those as
//! `false`, so consumers degrade *honestly* rather than silently dropping
//! them.

use objc2::rc::Retained;
use objc2::runtime::Bool;
use objc2_foundation::{NSBundle, NSError, NSProcessInfo, NSString, NSUUID};
use objc2_user_notifications::{
    UNAuthorizationOptions, UNMutableNotificationContent, UNNotificationInterruptionLevel,
    UNNotificationRequest, UNNotificationSound, UNUserNotificationCenter,
};
use std::sync::Once;

use crate::backend::{NotificationBackend, TsuuchiError};
use crate::notification::{Capabilities, Notification, NotificationSound, Urgency};

/// Native `UNUserNotificationCenter` backend. Zero-sized: it stores no
/// `Retained` handle (each `send` fetches the shared center), so it is
/// `Send + Sync` and satisfies [`NotificationBackend`].
pub struct UnBackend;

static AUTH_ONCE: Once = Once::new();

impl UnBackend {
    /// Construct the native backend, or `None` when the host process is
    /// not running bundled (no `CFBundleIdentifier` →
    /// `UNUserNotificationCenter` would abort). On success, requests
    /// authorization exactly once per process.
    ///
    /// Returning `Option` rather than `Result` is deliberate: "unbundled"
    /// is a normal, expected deployment (a plain CLI binary), not an
    /// error — the caller falls back to [`crate::LogBackend`].
    #[must_use]
    pub fn try_new() -> Option<Self> {
        // The single load-bearing precondition: a bundle identifier.
        let bundled = NSBundle::mainBundle().bundleIdentifier().is_some();
        if !bundled {
            tracing::info!(
                "host process is not bundled (no CFBundleIdentifier) — native \
                 UNUserNotificationCenter unavailable; falling back"
            );
            return None;
        }
        AUTH_ONCE.call_once(request_authorization);
        Some(Self)
    }
}

/// Request Alert+Sound+Badge authorization once per process. The macOS
/// prompt is the normal "<App> would like to send you notifications",
/// attributed to the host app — not the Script-Editor automation popup.
fn request_authorization() {
    let center = UNUserNotificationCenter::currentNotificationCenter();
    let opts =
        UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound | UNAuthorizationOptions::Badge;
    let handler = block2::RcBlock::new(|granted: Bool, _err: *mut NSError| {
        tracing::debug!(granted = granted.as_bool(), "UN authorization result");
    });
    center.requestAuthorizationWithOptions_completionHandler(opts, &handler);
}

/// `setInterruptionLevel:` requires macOS 12+; on 11 the selector is
/// unrecognised (would abort). Gate on the running OS major version.
fn supports_interruption_level() -> bool {
    NSProcessInfo::processInfo().operatingSystemVersion().majorVersion >= 12
}

/// Map tsuuchi urgency to a macOS interruption level. `Critical` maps to
/// `TimeSensitive` (pierces Focus, no entitlement) rather than the true
/// `Critical` level (which needs an Apple entitlement).
fn interruption_level(urgency: Urgency) -> UNNotificationInterruptionLevel {
    match urgency {
        Urgency::Low => UNNotificationInterruptionLevel::Passive,
        Urgency::Normal => UNNotificationInterruptionLevel::Active,
        Urgency::Critical => UNNotificationInterruptionLevel::TimeSensitive,
    }
}

/// Resolve a tsuuchi sound to a `UNNotificationSound` (or `None` for
/// silent).
fn sound_for(sound: &NotificationSound) -> Option<Retained<UNNotificationSound>> {
    match sound {
        NotificationSound::Silent => None,
        NotificationSound::Default => Some(UNNotificationSound::defaultSound()),
        NotificationSound::Critical => Some(UNNotificationSound::defaultCriticalSound()),
        NotificationSound::Named(name) => {
            let ns = NSString::from_str(name);
            Some(UNNotificationSound::soundNamed(&ns))
        }
    }
}

impl NotificationBackend for UnBackend {
    fn send(&self, n: &Notification) -> Result<(), TsuuchiError> {
        // Honest partial mapping — carried in the vocabulary, not yet
        // delivered on the native path (needs the delegate response path).
        if !n.actions.is_empty() {
            tracing::debug!(
                count = n.actions.len(),
                "UN backend: actions carried but not yet routed"
            );
        }
        if n.attachment.is_some() {
            tracing::debug!("UN backend: attachment carried but not yet delivered");
        }

        let content = UNMutableNotificationContent::new();
        content.setTitle(&NSString::from_str(&n.title));
        content.setBody(&NSString::from_str(&n.body));
        if let Some(sub) = &n.subtitle {
            content.setSubtitle(&NSString::from_str(sub));
        }
        content.setSound(sound_for(&n.sound).as_deref());
        if let Some(group) = &n.group {
            content.setThreadIdentifier(&NSString::from_str(group));
        }
        if let Some(category) = &n.category {
            content.setCategoryIdentifier(&NSString::from_str(category));
        }
        if supports_interruption_level() {
            content.setInterruptionLevel(interruption_level(n.urgency));
        }

        // Stable id → update-in-place; otherwise a fresh UUID per send.
        let identifier = match &n.id {
            Some(id) => NSString::from_str(id),
            None => NSUUID::UUID().UUIDString(),
        };
        let request =
            UNNotificationRequest::requestWithIdentifier_content_trigger(&identifier, &content, None);

        let center = UNUserNotificationCenter::currentNotificationCenter();
        center.addNotificationRequest_withCompletionHandler(&request, None);
        Ok(())
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            actions: false, // needs the delegate response path
            sound: true,
            attachments: false, // needs the delegate response path
            interruption_levels: supports_interruption_level(),
            reply: false, // needs the delegate response path
            update_in_place: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The urgency → interruption-level map is total and order-preserving:
    /// a more urgent notification never maps to a *less* intrusive level.
    /// `Critical` deliberately lands on `TimeSensitive`, not the true
    /// `Critical` level, which needs an Apple entitlement we do not hold —
    /// claiming it would abort at runtime.
    #[test]
    fn urgency_maps_to_interruption_level_monotonically() {
        assert_eq!(
            interruption_level(Urgency::Low),
            UNNotificationInterruptionLevel::Passive
        );
        assert_eq!(
            interruption_level(Urgency::Normal),
            UNNotificationInterruptionLevel::Active
        );
        assert_eq!(
            interruption_level(Urgency::Critical),
            UNNotificationInterruptionLevel::TimeSensitive
        );
    }

    /// Capabilities must report the UNROUTED axes as `false`. This is the
    /// tier-honesty gate: if someone wires the delegate response path and
    /// forgets to flip these, consumers would silently believe actions
    /// work. Conversely, flipping one to `true` without implementing it
    /// fails here.
    #[test]
    fn capabilities_are_tier_honest_about_unrouted_axes() {
        let caps = UnBackend.capabilities();
        assert!(!caps.actions, "actions are not routed yet");
        assert!(!caps.attachments, "attachments are not delivered yet");
        assert!(!caps.reply, "inline reply is not routed yet");
        // What the backend genuinely does deliver:
        assert!(caps.sound);
        assert!(caps.update_in_place, "stable id → update-in-place");
    }
}
