//! Notification history with bounded storage.
//!
//! Stores recent notifications in a ring buffer with timestamps,
//! allowing retrieval of past notifications for display in a
//! notification center or log.

use std::collections::VecDeque;
use std::time::Instant;

use crate::notification::Notification;

/// A timestamped notification entry.
#[derive(Debug, Clone)]
pub struct HistoryEntry {
    /// The notification that was sent.
    pub notification: Notification,
    /// When the notification was recorded.
    pub timestamp: Instant,
}

/// Bounded storage for recent notifications.
///
/// When the buffer reaches capacity, the oldest entry is evicted.
#[derive(Debug)]
pub struct NotificationHistory {
    entries: VecDeque<HistoryEntry>,
    capacity: usize,
}

impl NotificationHistory {
    /// Create a new history with the given maximum capacity.
    ///
    /// # Panics
    ///
    /// Panics if `capacity` is zero.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "notification history capacity must be > 0");
        Self {
            entries: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    /// Record a notification in the history.
    ///
    /// If the buffer is at capacity, the oldest entry is evicted.
    pub fn push(&mut self, notification: Notification) {
        if self.entries.len() == self.capacity {
            self.entries.pop_front();
        }
        self.entries.push_back(HistoryEntry {
            notification,
            timestamp: Instant::now(),
        });
    }

    /// Return the `n` most recent entries, newest first.
    #[must_use]
    pub fn recent(&self, n: usize) -> Vec<&HistoryEntry> {
        self.entries.iter().rev().take(n).collect()
    }

    /// Return the total number of entries stored.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Return whether the history is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear all stored entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notification::Urgency;

    fn make_notification(title: &str) -> Notification {
        Notification::new(title, "body")
    }

    #[test]
    fn push_and_recent() {
        let mut h = NotificationHistory::new(10);
        h.push(make_notification("first"));
        h.push(make_notification("second"));
        h.push(make_notification("third"));

        let recent = h.recent(2);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].notification.title, "third");
        assert_eq!(recent[1].notification.title, "second");
    }

    #[test]
    fn recent_more_than_available() {
        let mut h = NotificationHistory::new(10);
        h.push(make_notification("only"));
        let recent = h.recent(5);
        assert_eq!(recent.len(), 1);
    }

    #[test]
    fn ring_buffer_overflow() {
        let mut h = NotificationHistory::new(2);
        h.push(make_notification("a"));
        h.push(make_notification("b"));
        h.push(make_notification("c")); // evicts "a"

        assert_eq!(h.len(), 2);
        let recent = h.recent(10);
        assert_eq!(recent[0].notification.title, "c");
        assert_eq!(recent[1].notification.title, "b");
    }

    #[test]
    fn clear_history() {
        let mut h = NotificationHistory::new(10);
        h.push(make_notification("a"));
        h.push(make_notification("b"));
        assert!(!h.is_empty());

        h.clear();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn entries_have_timestamps() {
        let mut h = NotificationHistory::new(10);
        h.push(make_notification("timed"));

        let recent = h.recent(1);
        // Timestamp should be very recent (within last second)
        assert!(recent[0].timestamp.elapsed().as_secs() < 1);
    }

    #[test]
    fn preserves_notification_fields() {
        let mut h = NotificationHistory::new(10);
        h.push(
            Notification::new("Alert", "Details")
                .subtitle("Sub")
                .urgency(Urgency::Critical)
                .group("test"),
        );

        let entry = &h.recent(1)[0].notification;
        assert_eq!(entry.title, "Alert");
        assert_eq!(entry.body, "Details");
        assert_eq!(entry.subtitle.as_deref(), Some("Sub"));
        assert_eq!(entry.urgency, Urgency::Critical);
        assert_eq!(entry.group.as_deref(), Some("test"));
    }

    #[test]
    #[should_panic(expected = "capacity must be > 0")]
    fn zero_capacity_panics() {
        let _ = NotificationHistory::new(0);
    }

    #[test]
    fn capacity_of_one() {
        let mut h = NotificationHistory::new(1);
        h.push(make_notification("first"));
        assert_eq!(h.len(), 1);

        h.push(make_notification("second"));
        assert_eq!(h.len(), 1);

        let recent = h.recent(10);
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].notification.title, "second");
    }

    #[test]
    fn new_history_is_empty() {
        let h = NotificationHistory::new(5);
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn recent_zero_returns_empty() {
        let mut h = NotificationHistory::new(10);
        h.push(make_notification("a"));
        let recent = h.recent(0);
        assert!(recent.is_empty());
    }

    #[test]
    fn recent_on_empty_history() {
        let h = NotificationHistory::new(10);
        let recent = h.recent(5);
        assert!(recent.is_empty());
    }

    #[test]
    fn len_tracks_insertions() {
        let mut h = NotificationHistory::new(10);
        for i in 0..7 {
            h.push(make_notification(&format!("n{i}")));
            assert_eq!(h.len(), i + 1);
        }
    }

    #[test]
    fn len_does_not_exceed_capacity() {
        let mut h = NotificationHistory::new(3);
        for i in 0..10 {
            h.push(make_notification(&format!("n{i}")));
        }
        assert_eq!(h.len(), 3);
    }

    #[test]
    fn is_empty_after_push_then_clear() {
        let mut h = NotificationHistory::new(5);
        assert!(h.is_empty());

        h.push(make_notification("a"));
        assert!(!h.is_empty());

        h.clear();
        assert!(h.is_empty());
    }

    #[test]
    fn clear_then_reuse() {
        let mut h = NotificationHistory::new(3);
        h.push(make_notification("a"));
        h.push(make_notification("b"));
        h.clear();

        h.push(make_notification("c"));
        assert_eq!(h.len(), 1);

        let recent = h.recent(10);
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].notification.title, "c");
    }

    #[test]
    fn recent_returns_correct_order_newest_first() {
        let mut h = NotificationHistory::new(5);
        for label in ["a", "b", "c", "d", "e"] {
            h.push(make_notification(label));
        }

        let recent = h.recent(5);
        assert_eq!(recent[0].notification.title, "e");
        assert_eq!(recent[1].notification.title, "d");
        assert_eq!(recent[2].notification.title, "c");
        assert_eq!(recent[3].notification.title, "b");
        assert_eq!(recent[4].notification.title, "a");
    }

    #[test]
    fn ring_buffer_full_cycle() {
        let mut h = NotificationHistory::new(3);
        // Push 9 items, overwriting twice completely.
        for i in 0..9 {
            h.push(make_notification(&format!("n{i}")));
        }
        assert_eq!(h.len(), 3);

        let recent = h.recent(3);
        assert_eq!(recent[0].notification.title, "n8");
        assert_eq!(recent[1].notification.title, "n7");
        assert_eq!(recent[2].notification.title, "n6");
    }

    #[test]
    fn timestamps_are_non_decreasing() {
        let mut h = NotificationHistory::new(10);
        h.push(make_notification("first"));
        h.push(make_notification("second"));
        h.push(make_notification("third"));

        let recent = h.recent(3);
        // recent is newest-first, so timestamps should be non-increasing.
        assert!(recent[0].timestamp >= recent[1].timestamp);
        assert!(recent[1].timestamp >= recent[2].timestamp);
    }

    #[test]
    fn history_entry_clone() {
        let mut h = NotificationHistory::new(5);
        h.push(
            Notification::new("Clone", "Test")
                .subtitle("S")
                .urgency(Urgency::Critical)
                .group("g"),
        );

        let entry = h.recent(1)[0].clone();
        assert_eq!(entry.notification.title, "Clone");
        assert_eq!(entry.notification.urgency, Urgency::Critical);
    }

    #[test]
    fn history_preserves_all_urgency_levels() {
        let mut h = NotificationHistory::new(10);
        h.push(Notification::new("Low", "b").urgency(Urgency::Low));
        h.push(Notification::new("Normal", "b").urgency(Urgency::Normal));
        h.push(Notification::new("Critical", "b").urgency(Urgency::Critical));

        let recent = h.recent(3);
        assert_eq!(recent[0].notification.urgency, Urgency::Critical);
        assert_eq!(recent[1].notification.urgency, Urgency::Normal);
        assert_eq!(recent[2].notification.urgency, Urgency::Low);
    }

    #[test]
    fn large_capacity_history() {
        let cap = 1000;
        let mut h = NotificationHistory::new(cap);
        for i in 0..cap {
            h.push(make_notification(&format!("n{i}")));
        }
        assert_eq!(h.len(), cap);

        // Push one more to trigger eviction.
        h.push(make_notification("overflow"));
        assert_eq!(h.len(), cap);

        let recent = h.recent(1);
        assert_eq!(recent[0].notification.title, "overflow");
    }
}
