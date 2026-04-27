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

### Adding a Platform Backend

```rust
use tsuuchi::{NotificationBackend, Notification, TsuuchiError};

struct MacOSBackend;

impl NotificationBackend for MacOSBackend {
    fn send(&self, notification: &Notification) -> Result<(), TsuuchiError> {
        // Use NSUserNotification or UNUserNotificationCenter
        todo!()
    }
}
```

## Consumers

- **tobira** — app launcher notifications
- **hikyaku** — new email notifications
- **ayatsuri** — window manager status alerts
