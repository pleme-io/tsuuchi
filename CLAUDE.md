# Tsuuchi (通知) — Notification Framework

> **★★★ CSE / Knowable Construction.** This repo operates under **Constructive Substrate Engineering** — canonical specification at [`pleme-io/theory/CONSTRUCTIVE-SUBSTRATE-ENGINEERING.md`](https://github.com/pleme-io/theory/blob/main/CONSTRUCTIVE-SUBSTRATE-ENGINEERING.md). The Compounding Directive (operational rules: solve once, load-bearing fixes only, idiom-first, models stay current, direction beats velocity) is in the org-level pleme-io/CLAUDE.md ★★★ section. Read both before non-trivial changes.


## Build & Test

```bash
cargo build          # compile
cargo test           # 16 unit tests + 2 doc-tests
```

## Architecture

Platform-agnostic notification framework with:
- Builder-pattern notification construction with urgency levels
- Trait-based backend system for pluggable platform support
- Log-only fallback backend for testing and headless environments
- Dispatcher routing notifications through a configured backend
- Bounded notification history with timestamps

### Module Map

| Path | Purpose |
|------|---------|
| `src/lib.rs` | Re-exports Notification, Urgency, backends, dispatcher, history |
| `src/notification.rs` | `Notification` struct + `Urgency` enum, builder pattern (5 tests) |
| `src/backend.rs` | `NotificationBackend` trait + `LogBackend` (3 tests) |
| `src/backend_macos.rs` | `UnBackend` — native `UNUserNotificationCenter` (macOS only, 2 tests) |
| `src/dispatcher.rs` | `NotificationDispatcher` — routes through backend (2 tests) |
| `src/history.rs` | `NotificationHistory` — timestamped ring buffer (6 tests) |

### Key Types

- **`Notification`** — title, body, subtitle, urgency, group with builder pattern
- **`Urgency`** — Low, Normal (default), Critical (ordered)
- **`NotificationBackend`** — trait: `fn send(&self, notification) -> Result<(), TsuuchiError>`
- **`LogBackend`** — logs via tracing (default fallback)
- **`NotificationDispatcher`** — owns `Box<dyn NotificationBackend>`, delegates `send()`
- **`NotificationHistory`** — `VecDeque<HistoryEntry>` with `Instant` timestamps
- **`HistoryEntry`** — `{ notification: Notification, timestamp: Instant }`

### Usage Pattern

```rust
use tsuuchi::{Notification, Urgency, NotificationDispatcher, LogBackend};

let dispatcher = NotificationDispatcher::new(Box::new(LogBackend::new()));

dispatcher.send(
    &Notification::new("Build Done", "42 tests passed")
        .urgency(Urgency::Low)
        .group("ci")
).unwrap();
```

### macOS — native, shipped

`UnBackend` (`src/backend_macos.rs`) is the real `UNUserNotificationCenter`
binding. It is `cfg(target_os = "macos")`-gated and its objc2 deps are
declared under `[target.'cfg(target_os = "macos")'.dependencies]`, so a
Linux/CI build never pulls them and there is no cargo feature to remember.

```rust
// Native when bundled; honest fallback when not.
let backend: Box<dyn tsuuchi::NotificationBackend> = tsuuchi::UnBackend::try_new()
    .map(|b| Box::new(b) as _)
    .unwrap_or_else(|| Box::new(tsuuchi::LogBackend::new()));
let dispatcher = tsuuchi::NotificationDispatcher::new(backend);
```

`try_new()` returns `None` when the host process has no
`CFBundleIdentifier` — `UNUserNotificationCenter` *aborts* for an
unbundled process, so this is a hard precondition, not a preference.
Unbundled is a normal deployment (a plain CLI), which is why it is
`Option` and not `Result`.

**Tier honesty — read `capabilities()`, do not assume.** Delivered:
content, sound, interruption level (macOS 12+), thread grouping, and
stable-id update-in-place. NOT delivered: `actions`, `attachments`,
`reply` — they travel in the vocabulary but need a
`UNUserNotificationCenterDelegate` response path (a retained delegate +
an app run-loop). `capabilities()` reports each as `false`;
`capabilities_are_tier_honest_about_unrouted_axes` fails the build if one
is flipped without being implemented.

**Why it lives here:** this binding previously lived in **mado**, a
consumer, leaving the fleet's notification primitive hollow on its
primary platform. Owning it here is the solve-once fix — every consumer
below now gets native macOS notifications rather than re-rolling objc2.

### Adding another platform backend

Implement `NotificationBackend` (see `backend_macos.rs` as the worked
reference) and override `capabilities()` with what the platform *actually*
delivers — an unsupported axis must be traced and reported `false`, never
silently dropped.

## Consumers

- **tobira** — app launcher notifications
- **hikyaku** — new email notifications
- **ayatsuri** — window manager status alerts
- **mado** — terminal bell / command-done banners (native `UnBackend`)
