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
}
