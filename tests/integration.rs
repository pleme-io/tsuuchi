//! Integration tests for tsuuchi — notification construction, dispatch,
//! history, and backend interaction across module boundaries.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use tsuuchi::{
    LogBackend, Notification, NotificationBackend, NotificationDispatcher, NotificationHistory,
    TsuuchiError, Urgency,
};

// ---------------------------------------------------------------------------
// Dispatcher + History integration
// ---------------------------------------------------------------------------

/// A backend that records all notifications it receives.
struct RecordingBackend {
    sent: Mutex<Vec<Notification>>,
}

impl RecordingBackend {
    fn new() -> Self {
        Self {
            sent: Mutex::new(Vec::new()),
        }
    }

    fn sent_count(&self) -> usize {
        self.sent.lock().unwrap().len()
    }

    fn last(&self) -> Option<Notification> {
        self.sent.lock().unwrap().last().cloned()
    }
}

impl NotificationBackend for RecordingBackend {
    fn send(&self, notification: &Notification) -> Result<(), TsuuchiError> {
        self.sent.lock().unwrap().push(notification.clone());
        Ok(())
    }
}

/// Arc wrapper so RecordingBackend can be shared with the dispatcher.
struct ArcBackend(Arc<RecordingBackend>);

impl NotificationBackend for ArcBackend {
    fn send(&self, notification: &Notification) -> Result<(), TsuuchiError> {
        self.0.send(notification)
    }
}

#[test]
fn dispatcher_sends_and_history_records() {
    let backend = Arc::new(RecordingBackend::new());
    let dispatcher = NotificationDispatcher::new(Box::new(ArcBackend(Arc::clone(&backend))));
    let mut history = NotificationHistory::new(50);

    let notifications = [
        Notification::new("Build", "Success").urgency(Urgency::Low).group("ci"),
        Notification::new("Deploy", "Started").urgency(Urgency::Normal).group("ci"),
        Notification::new("Alert", "CPU 95%").urgency(Urgency::Critical).group("monitoring"),
    ];

    for n in &notifications {
        dispatcher.send(n).unwrap();
        history.push(n.clone());
    }

    assert_eq!(backend.sent_count(), 3);
    assert_eq!(history.len(), 3);

    // History recent is newest-first
    let recent = history.recent(3);
    assert_eq!(recent[0].notification.title, "Alert");
    assert_eq!(recent[1].notification.title, "Deploy");
    assert_eq!(recent[2].notification.title, "Build");
}

#[test]
fn failed_dispatch_does_not_affect_history() {
    struct FailBackend;
    impl NotificationBackend for FailBackend {
        fn send(&self, _: &Notification) -> Result<(), TsuuchiError> {
            Err(TsuuchiError::SendFailed("backend down".into()))
        }
    }

    let dispatcher = NotificationDispatcher::new(Box::new(FailBackend));
    let mut history = NotificationHistory::new(10);

    let n = Notification::new("Test", "Body");

    // Dispatch fails
    let result = dispatcher.send(&n);
    assert!(result.is_err());

    // But we can still record it in history independently
    history.push(n.clone());
    assert_eq!(history.len(), 1);
}

// ---------------------------------------------------------------------------
// Notification builder across dispatch pipeline
// ---------------------------------------------------------------------------

#[test]
fn full_notification_survives_dispatch_pipeline() {
    let backend = Arc::new(RecordingBackend::new());
    let dispatcher = NotificationDispatcher::new(Box::new(ArcBackend(Arc::clone(&backend))));

    let original = Notification::new("Title", "Body")
        .subtitle("Subtitle")
        .urgency(Urgency::Critical)
        .group("test-group");

    dispatcher.send(&original).unwrap();

    let received = backend.last().unwrap();
    assert_eq!(received.title, "Title");
    assert_eq!(received.body, "Body");
    assert_eq!(received.subtitle.as_deref(), Some("Subtitle"));
    assert_eq!(received.urgency, Urgency::Critical);
    assert_eq!(received.group.as_deref(), Some("test-group"));
    assert_eq!(received, original);
}

#[test]
fn minimal_notification_survives_dispatch() {
    let backend = Arc::new(RecordingBackend::new());
    let dispatcher = NotificationDispatcher::new(Box::new(ArcBackend(Arc::clone(&backend))));

    let n = Notification::new("T", "B");
    dispatcher.send(&n).unwrap();

    let received = backend.last().unwrap();
    assert_eq!(received.subtitle, None);
    assert_eq!(received.urgency, Urgency::Normal);
    assert_eq!(received.group, None);
}

// ---------------------------------------------------------------------------
// History eviction with full notification data
// ---------------------------------------------------------------------------

#[test]
fn history_eviction_preserves_newest() {
    let mut history = NotificationHistory::new(3);

    for i in 0..10 {
        history.push(
            Notification::new(format!("Title-{i}"), format!("Body-{i}"))
                .urgency(if i % 2 == 0 {
                    Urgency::Low
                } else {
                    Urgency::Critical
                }),
        );
    }

    assert_eq!(history.len(), 3);
    let recent = history.recent(3);
    assert_eq!(recent[0].notification.title, "Title-9");
    assert_eq!(recent[1].notification.title, "Title-8");
    assert_eq!(recent[2].notification.title, "Title-7");

    // Urgency is preserved correctly
    assert_eq!(recent[0].notification.urgency, Urgency::Critical);
    assert_eq!(recent[1].notification.urgency, Urgency::Low);
    assert_eq!(recent[2].notification.urgency, Urgency::Critical);
}

// ---------------------------------------------------------------------------
// Urgency-based routing
// ---------------------------------------------------------------------------

#[test]
fn urgency_based_backend_selection() {
    struct UrgencyCountingBackend {
        low: AtomicUsize,
        normal: AtomicUsize,
        critical: AtomicUsize,
    }

    impl UrgencyCountingBackend {
        fn new() -> Self {
            Self {
                low: AtomicUsize::new(0),
                normal: AtomicUsize::new(0),
                critical: AtomicUsize::new(0),
            }
        }
    }

    impl NotificationBackend for UrgencyCountingBackend {
        fn send(&self, notification: &Notification) -> Result<(), TsuuchiError> {
            match notification.urgency {
                Urgency::Low => self.low.fetch_add(1, Ordering::Relaxed),
                Urgency::Normal => self.normal.fetch_add(1, Ordering::Relaxed),
                Urgency::Critical => self.critical.fetch_add(1, Ordering::Relaxed),
            };
            Ok(())
        }
    }

    let backend = Arc::new(UrgencyCountingBackend::new());

    struct ArcUrgency(Arc<UrgencyCountingBackend>);
    impl NotificationBackend for ArcUrgency {
        fn send(&self, n: &Notification) -> Result<(), TsuuchiError> {
            self.0.send(n)
        }
    }

    let dispatcher = NotificationDispatcher::new(Box::new(ArcUrgency(Arc::clone(&backend))));

    dispatcher
        .send(&Notification::new("a", "b").urgency(Urgency::Low))
        .unwrap();
    dispatcher
        .send(&Notification::new("a", "b").urgency(Urgency::Low))
        .unwrap();
    dispatcher
        .send(&Notification::new("a", "b").urgency(Urgency::Normal))
        .unwrap();
    dispatcher
        .send(&Notification::new("a", "b").urgency(Urgency::Critical))
        .unwrap();
    dispatcher
        .send(&Notification::new("a", "b").urgency(Urgency::Critical))
        .unwrap();
    dispatcher
        .send(&Notification::new("a", "b").urgency(Urgency::Critical))
        .unwrap();

    assert_eq!(backend.low.load(Ordering::Relaxed), 2);
    assert_eq!(backend.normal.load(Ordering::Relaxed), 1);
    assert_eq!(backend.critical.load(Ordering::Relaxed), 3);
}

// ---------------------------------------------------------------------------
// Unicode and special content through the full pipeline
// ---------------------------------------------------------------------------

#[test]
fn unicode_through_dispatch_and_history() {
    let backend = Arc::new(RecordingBackend::new());
    let dispatcher = NotificationDispatcher::new(Box::new(ArcBackend(Arc::clone(&backend))));
    let mut history = NotificationHistory::new(10);

    let n = Notification::new("通知テスト", "本文 🔔")
        .subtitle("サブタイトル")
        .group("グループ");

    dispatcher.send(&n).unwrap();
    history.push(n.clone());

    let received = backend.last().unwrap();
    assert_eq!(received.title, "通知テスト");
    assert_eq!(received.body, "本文 🔔");
    assert_eq!(received.subtitle.as_deref(), Some("サブタイトル"));
    assert_eq!(received.group.as_deref(), Some("グループ"));

    let entry = &history.recent(1)[0];
    assert_eq!(entry.notification, n);
}

// ---------------------------------------------------------------------------
// Serialization round-trip through pipeline
// ---------------------------------------------------------------------------

#[test]
fn notification_serialization_roundtrip_through_history() {
    let mut history = NotificationHistory::new(10);

    let original = Notification::new("Serialize", "Test")
        .subtitle("Sub")
        .urgency(Urgency::Critical)
        .group("g");

    // Serialize, then deserialize to simulate persistence
    let json = serde_json::to_string(&original).unwrap();
    let deserialized: Notification = serde_json::from_str(&json).unwrap();

    history.push(deserialized);

    let entry = &history.recent(1)[0].notification;
    assert_eq!(entry, &original);
}

#[test]
fn all_urgency_levels_serialize_correctly() {
    for urgency in [Urgency::Low, Urgency::Normal, Urgency::Critical] {
        let n = Notification::new("T", "B").urgency(urgency);
        let json = serde_json::to_string(&n).unwrap();
        let restored: Notification = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.urgency, urgency);
    }
}

// ---------------------------------------------------------------------------
// Error handling across modules
// ---------------------------------------------------------------------------

#[test]
fn error_types_are_distinguishable() {
    let send_err = TsuuchiError::SendFailed("test".into());
    let unavail_err = TsuuchiError::Unavailable("test".into());

    assert!(matches!(send_err, TsuuchiError::SendFailed(_)));
    assert!(matches!(unavail_err, TsuuchiError::Unavailable(_)));

    // They produce different display strings
    assert_ne!(send_err.to_string(), unavail_err.to_string());
}

#[test]
fn dispatcher_error_propagation_with_context() {
    struct ContextBackend;
    impl NotificationBackend for ContextBackend {
        fn send(&self, n: &Notification) -> Result<(), TsuuchiError> {
            Err(TsuuchiError::SendFailed(format!(
                "failed to deliver '{}' to group {:?}",
                n.title, n.group
            )))
        }
    }

    let dispatcher = NotificationDispatcher::new(Box::new(ContextBackend));
    let n = Notification::new("Alert", "Body").group("ops");
    let err = dispatcher.send(&n).unwrap_err();

    let msg = err.to_string();
    assert!(msg.contains("Alert"));
    assert!(msg.contains("ops"));
}

// ---------------------------------------------------------------------------
// LogBackend integration
// ---------------------------------------------------------------------------

#[test]
fn log_backend_accepts_all_notification_variations() {
    let dispatcher = NotificationDispatcher::new(Box::new(LogBackend::new()));

    let variations = [
        Notification::new("Minimal", "Body"),
        Notification::new("With Sub", "Body").subtitle("S"),
        Notification::new("With Group", "Body").group("G"),
        Notification::new("Full", "Body")
            .subtitle("S")
            .urgency(Urgency::Critical)
            .group("G"),
        Notification::new("", ""),
        Notification::new("Unicode 🔔", "テスト"),
    ];

    for n in &variations {
        assert!(dispatcher.send(n).is_ok());
    }
}

// ---------------------------------------------------------------------------
// History boundary conditions
// ---------------------------------------------------------------------------

#[test]
fn history_clear_and_reuse() {
    let mut history = NotificationHistory::new(5);

    for i in 0..5 {
        history.push(Notification::new(format!("batch1-{i}"), "b"));
    }
    assert_eq!(history.len(), 5);

    history.clear();
    assert!(history.is_empty());

    for i in 0..3 {
        history.push(Notification::new(format!("batch2-{i}"), "b"));
    }
    assert_eq!(history.len(), 3);

    let recent = history.recent(3);
    assert_eq!(recent[0].notification.title, "batch2-2");
}

#[test]
fn history_timestamps_are_monotonic() {
    let mut history = NotificationHistory::new(100);

    for i in 0..20 {
        history.push(Notification::new(format!("n{i}"), "b"));
    }

    let recent = history.recent(20);
    for window in recent.windows(2) {
        // recent is newest-first, so timestamps should be non-increasing
        assert!(window[0].timestamp >= window[1].timestamp);
    }
}

// ---------------------------------------------------------------------------
// High throughput
// ---------------------------------------------------------------------------

#[test]
fn high_throughput_dispatch_and_history() {
    let backend = Arc::new(RecordingBackend::new());
    let dispatcher = NotificationDispatcher::new(Box::new(ArcBackend(Arc::clone(&backend))));
    let mut history = NotificationHistory::new(100);

    for i in 0..1000 {
        let n = Notification::new(format!("n{i}"), "body").urgency(match i % 3 {
            0 => Urgency::Low,
            1 => Urgency::Normal,
            _ => Urgency::Critical,
        });
        dispatcher.send(&n).unwrap();
        history.push(n);
    }

    assert_eq!(backend.sent_count(), 1000);
    assert_eq!(history.len(), 100);

    let recent = history.recent(1);
    assert_eq!(recent[0].notification.title, "n999");
}
