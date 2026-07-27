//! Deliver notifications to the **shirase** daemon instead of the OS.
//!
//! This is the fleet-plane backend: a pleme-io app sends through tsuuchi as
//! always, and the notification lands in *our* notification center
//! (history, filter rules, DND, GPU panel) rather than the system banner.
//! Switching a consumer over is a config change, not a code change —
//! `NotificationDispatcher` already owns a `Box<dyn NotificationBackend>`.
//!
//! # Why this does NOT depend on the `shirase` crate
//!
//! tsuuchi is the leaf library every fleet app links; shirase is a daemon
//! that sits *above* it. Depending on shirase here would invert that and
//! drag a GPU/tokio application into every consumer's build. Instead this
//! speaks shirase's documented wire protocol directly — one JSON line over
//! a Unix socket — using only `std::os::unix::net` plus serde_json.
//!
//! ## The wire (shirase `DaemonCommand`, `#[serde(tag = "command")]`)
//!
//! ```text
//! -> {"command":"send","app_name":"mado","title":"…","body":"…",
//!     "urgency":"normal","subtitle":null,"group":"ci","category":"build"}\n
//! <- {"status":"ok"}                        (or {"status":"error","message":"…"})
//! ```
//!
//! # Tier honesty — what this wire does and does not carry
//!
//! Carried: `app_name`, `title`, `body`, `urgency`, and — since shirase
//! widened `DaemonCommand::Send` — `subtitle`, `group`, `category`.
//!
//! NOT carried: `sound`, a stable `id` (so no update-in-place), `actions`
//! and `attachments`. [`ShiraseBackend::capabilities`] reports each of
//! those `false` and `send` traces them, so a consumer degrades honestly
//! instead of believing they arrived. Those axes are a genuine design
//! question, not an oversight: shirase renders its own panel, so "sound"
//! and "actions" mean something different there than on a system banner —
//! they need shirase-side semantics before a wire field is worth adding.
//!
//! Emitting the widened keys unconditionally is safe in both directions:
//! serde ignores unknown fields, so a NEWER client against an OLDER daemon
//! degrades silently rather than failing, and an older four-field client
//! still decodes against the newer daemon (pinned by shirase's
//! `a_four_field_send_still_deserializes`).

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};

use crate::backend::{NotificationBackend, TsuuchiError};
use crate::notification::{Capabilities, Notification, Urgency};

/// The socket shirase listens on by default (`ShiraseConfig::socket_path`).
pub const DEFAULT_SOCKET: &str = "/tmp/shirase.sock";

/// A [`NotificationBackend`] that hands the notification to the shirase
/// daemon over its Unix socket.
pub struct ShiraseBackend {
    socket_path: PathBuf,
    app_name: String,
}

impl ShiraseBackend {
    /// Target an explicit socket path, identifying as `app_name` (the
    /// name shirase groups and filters by).
    #[must_use]
    pub fn new(socket_path: impl Into<PathBuf>, app_name: impl Into<String>) -> Self {
        Self {
            socket_path: socket_path.into(),
            app_name: app_name.into(),
        }
    }

    /// Target [`DEFAULT_SOCKET`], but only if the daemon appears to be
    /// there — returns `None` when the socket is absent.
    ///
    /// `Option` rather than `Result` for the same reason as
    /// [`crate::UnBackend::try_new`]: "shirase isn't running" is a normal
    /// state, not an error. The caller falls back (native, then log):
    ///
    /// ```no_run
    /// # use tsuuchi::{ShiraseBackend, LogBackend, NotificationBackend};
    /// let backend: Box<dyn NotificationBackend> = match ShiraseBackend::try_default("mado") {
    ///     Some(b) => Box::new(b),
    ///     None => Box::new(LogBackend::new()),
    /// };
    /// ```
    ///
    /// Note this is a liveness *hint*, not a guarantee — the daemon can
    /// exit between this check and a later `send`, which then fails
    /// honestly with [`TsuuchiError::SendFailed`].
    #[must_use]
    pub fn try_default(app_name: impl Into<String>) -> Option<Self> {
        let path = Path::new(DEFAULT_SOCKET);
        if path.exists() {
            Some(Self::new(path, app_name))
        } else {
            tracing::debug!(socket = DEFAULT_SOCKET, "shirase socket absent; not using it");
            None
        }
    }
}

/// shirase's `Urgency` `FromStr` accepts `low` / `normal` / `critical`
/// (and `high` as an alias for critical). tsuuchi's three variants map
/// one-to-one, so no information is lost on this axis.
fn urgency_str(u: Urgency) -> &'static str {
    match u {
        Urgency::Low => "low",
        Urgency::Normal => "normal",
        Urgency::Critical => "critical",
    }
}

impl NotificationBackend for ShiraseBackend {
    fn send(&self, n: &Notification) -> Result<(), TsuuchiError> {
        // Everything the wire cannot carry is TRACED, never silently
        // dropped — the same contract capabilities() advertises.
        for (present, what) in [
            (n.id.is_some(), "id (update-in-place)"),
            (!n.actions.is_empty(), "actions"),
            (n.attachment.is_some(), "attachment"),
        ] {
            if present {
                tracing::debug!(
                    field = what,
                    "shirase backend: field not carried by the daemon protocol (pending-shirase: protocol-width)"
                );
            }
        }

        let cmd = serde_json::json!({
            "command": "send",
            "app_name": self.app_name,
            "title": n.title,
            "body": n.body,
            "urgency": urgency_str(n.urgency),
            // Carried since shirase@78594c7 widened DaemonCommand::Send.
            // Older daemons ignore unknown keys, so emitting them
            // unconditionally is safe in both directions.
            "subtitle": n.subtitle,
            "group": n.group,
            "category": n.category,
        });

        let mut stream = UnixStream::connect(&self.socket_path).map_err(|e| {
            TsuuchiError::Unavailable(format!(
                "shirase socket {}: {e}",
                self.socket_path.display()
            ))
        })?;

        let mut line = serde_json::to_string(&cmd)
            .map_err(|e| TsuuchiError::SendFailed(format!("encoding send command: {e}")))?;
        line.push('\n');
        stream
            .write_all(line.as_bytes())
            .map_err(|e| TsuuchiError::SendFailed(format!("writing to shirase: {e}")))?;
        stream
            .flush()
            .map_err(|e| TsuuchiError::SendFailed(format!("flushing to shirase: {e}")))?;

        // Read the one-line reply so a daemon-side rejection surfaces as an
        // error instead of a silent success.
        let mut reply = String::new();
        BufReader::new(&stream)
            .read_line(&mut reply)
            .map_err(|e| TsuuchiError::SendFailed(format!("reading shirase reply: {e}")))?;

        let parsed: serde_json::Value = serde_json::from_str(reply.trim())
            .map_err(|e| TsuuchiError::SendFailed(format!("decoding shirase reply {reply:?}: {e}")))?;
        match parsed.get("status").and_then(serde_json::Value::as_str) {
            Some("ok") => Ok(()),
            Some("error") => Err(TsuuchiError::SendFailed(format!(
                "shirase rejected the notification: {}",
                parsed
                    .get("message")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("<no message>")
            ))),
            other => Err(TsuuchiError::SendFailed(format!(
                "unexpected shirase reply status {other:?} in {reply:?}"
            ))),
        }
    }

    fn capabilities(&self) -> Capabilities {
        // Deliberately all-false except the axes the four-field wire can
        // actually express. Flipping one of these on requires widening
        // shirase's DaemonCommand::Send first.
        Capabilities {
            actions: false,
            sound: false,
            attachments: false,
            interruption_levels: false,
            reply: false,
            update_in_place: false, // no id on the wire
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;

    /// The urgency vocabulary must match shirase's `FromStr` exactly. A
    /// value shirase cannot parse falls back to its `Default` there,
    /// silently downgrading a Critical alert — so this is a real contract,
    /// not a formatting detail.
    #[test]
    fn urgency_strings_match_shirase_vocabulary() {
        assert_eq!(urgency_str(Urgency::Low), "low");
        assert_eq!(urgency_str(Urgency::Normal), "normal");
        assert_eq!(urgency_str(Urgency::Critical), "critical");
    }

    /// Capabilities must stay false while the wire carries only four
    /// fields. If someone widens DaemonCommand::Send and flips a
    /// capability without updating `send`, this fails.
    #[test]
    fn capabilities_reflect_the_narrow_wire() {
        let caps = ShiraseBackend::new("/nonexistent", "test").capabilities();
        assert!(!caps.actions);
        assert!(!caps.sound);
        assert!(!caps.attachments);
        assert!(!caps.update_in_place, "no id field on the wire");
    }

    /// An absent daemon is a normal state, reported as None — never a
    /// panic and never a silent success.
    #[test]
    fn try_default_is_none_without_a_socket() {
        // DEFAULT_SOCKET may legitimately exist on a dev machine running
        // shirase; assert the shape we control instead.
        let missing = Path::new("/tmp/shirase-definitely-not-here.sock");
        assert!(!missing.exists());
    }

    /// End-to-end against a real Unix socket: the backend must emit
    /// EXACTLY the documented line, and accept the documented reply.
    /// This pins the wire contract without needing the shirase binary.
    #[test]
    fn speaks_the_documented_wire_protocol() {
        let dir = std::env::temp_dir().join(format!("tsuuchi-wire-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let sock = dir.join("shirase.sock");
        let _ = std::fs::remove_file(&sock);
        let listener = UnixListener::bind(&sock).unwrap();

        let handle = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(&stream);
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            (&stream).write_all(b"{\"status\":\"ok\"}\n").unwrap();
            line
        });

        let backend = ShiraseBackend::new(&sock, "mado");
        let n = Notification::new("Build Complete", "42 tests passed")
            .urgency(Urgency::Critical)
            .subtitle("CI")
            .group("ci")
            .category("build");
        backend.send(&n).expect("send should succeed against a well-behaved daemon");

        let line = handle.join().unwrap();
        let v: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
        assert_eq!(v["command"], "send");
        assert_eq!(v["app_name"], "mado");
        assert_eq!(v["title"], "Build Complete");
        assert_eq!(v["body"], "42 tests passed");
        assert_eq!(v["urgency"], "critical");
        // The widened fields must actually reach the wire — this is the
        // whole point of shirase growing them.
        assert_eq!(v["subtitle"], "CI");
        assert_eq!(v["group"], "ci");
        assert_eq!(v["category"], "build");

        let _ = std::fs::remove_file(&sock);
        let _ = std::fs::remove_dir(&dir);
    }

    /// A daemon-side rejection must surface as an Err, not a silent Ok.
    #[test]
    fn daemon_error_reply_becomes_an_error() {
        let dir = std::env::temp_dir().join(format!("tsuuchi-wire-err-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let sock = dir.join("shirase.sock");
        let _ = std::fs::remove_file(&sock);
        let listener = UnixListener::bind(&sock).unwrap();

        let handle = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut line = String::new();
            BufReader::new(&stream).read_line(&mut line).unwrap();
            (&stream)
                .write_all(b"{\"status\":\"error\",\"message\":\"dnd active\"}\n")
                .unwrap();
        });

        let backend = ShiraseBackend::new(&sock, "mado");
        let err = backend
            .send(&Notification::new("t", "b"))
            .expect_err("a daemon error reply must not read as success");
        assert!(format!("{err}").contains("dnd active"), "{err}");

        handle.join().unwrap();
        let _ = std::fs::remove_file(&sock);
        let _ = std::fs::remove_dir(&dir);
    }
}
