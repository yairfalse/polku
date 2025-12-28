//! Ring buffer for event buffering with backpressure support

use crate::proto::Event;
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};

/// Thread-safe ring buffer for events
///
/// When full, oldest events are dropped (FIFO eviction).
/// Provides metrics for monitoring buffer state.
pub struct RingBuffer {
    events: Mutex<VecDeque<Event>>,
    capacity: usize,
    metrics: BufferMetrics,
}

/// Metrics for buffer monitoring
pub struct BufferMetrics {
    /// Total events pushed
    pub pushed: AtomicU64,
    /// Total events dropped due to full buffer
    pub dropped: AtomicU64,
    /// Total events drained
    pub drained: AtomicU64,
}

impl Default for BufferMetrics {
    fn default() -> Self {
        Self {
            pushed: AtomicU64::new(0),
            dropped: AtomicU64::new(0),
            drained: AtomicU64::new(0),
        }
    }
}

impl RingBuffer {
    /// Create a new ring buffer with the given capacity
    pub fn new(capacity: usize) -> Self {
        Self {
            events: Mutex::new(VecDeque::with_capacity(capacity)),
            capacity,
            metrics: BufferMetrics::default(),
        }
    }

    /// Push events into the buffer
    ///
    /// Returns the number of events dropped due to capacity limits.
    pub fn push(&self, events: Vec<Event>) -> usize {
        let mut buffer = self.events.lock();
        let mut dropped = 0;

        for event in events {
            if buffer.len() >= self.capacity {
                // Drop oldest event (FIFO eviction)
                buffer.pop_front();
                dropped += 1;
            }
            buffer.push_back(event);
        }

        self.metrics
            .pushed
            .fetch_add((buffer.len() + dropped) as u64, Ordering::Relaxed);
        self.metrics
            .dropped
            .fetch_add(dropped as u64, Ordering::Relaxed);

        dropped
    }

    /// Drain up to `n` events from the buffer
    ///
    /// Returns the drained events in FIFO order.
    pub fn drain(&self, n: usize) -> Vec<Event> {
        let mut buffer = self.events.lock();
        let drain_count = n.min(buffer.len());
        let events: Vec<Event> = buffer.drain(..drain_count).collect();

        self.metrics
            .drained
            .fetch_add(events.len() as u64, Ordering::Relaxed);

        events
    }

    /// Get current number of events in buffer
    pub fn len(&self) -> usize {
        self.events.lock().len()
    }

    /// Check if buffer is empty
    pub fn is_empty(&self) -> bool {
        self.events.lock().is_empty()
    }

    /// Check if buffer is at capacity
    pub fn is_full(&self) -> bool {
        self.events.lock().len() >= self.capacity
    }

    /// Get buffer capacity
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Get current fill percentage (0.0 - 1.0)
    pub fn fill_ratio(&self) -> f64 {
        let len = self.events.lock().len();
        len as f64 / self.capacity as f64
    }

    /// Get total events pushed
    pub fn total_pushed(&self) -> u64 {
        self.metrics.pushed.load(Ordering::Relaxed)
    }

    /// Get total events dropped
    pub fn total_dropped(&self) -> u64 {
        self.metrics.dropped.load(Ordering::Relaxed)
    }

    /// Get total events drained
    pub fn total_drained(&self) -> u64 {
        self.metrics.drained.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use prost_types::Timestamp;

    fn make_event(id: &str) -> Event {
        Event {
            id: id.to_string(),
            timestamp: Some(Timestamp::default()),
            ..Default::default()
        }
    }

    #[test]
    fn test_push_and_drain() {
        let buffer = RingBuffer::new(10);

        // Push 5 events
        let events: Vec<Event> = (0..5).map(|i| make_event(&format!("event-{i}"))).collect();
        let dropped = buffer.push(events);

        assert_eq!(dropped, 0);
        assert_eq!(buffer.len(), 5);

        // Drain 3 events
        let drained = buffer.drain(3);
        assert_eq!(drained.len(), 3);
        assert_eq!(drained[0].id, "event-0");
        assert_eq!(drained[2].id, "event-2");
        assert_eq!(buffer.len(), 2);
    }

    #[test]
    fn test_overflow_drops_oldest() {
        let buffer = RingBuffer::new(3);

        // Push 5 events into a buffer of size 3
        let events: Vec<Event> = (0..5).map(|i| make_event(&format!("event-{i}"))).collect();
        let dropped = buffer.push(events);

        assert_eq!(dropped, 2); // 2 events dropped
        assert_eq!(buffer.len(), 3);

        // Should have events 2, 3, 4 (oldest 0, 1 dropped)
        let drained = buffer.drain(3);
        assert_eq!(drained[0].id, "event-2");
        assert_eq!(drained[1].id, "event-3");
        assert_eq!(drained[2].id, "event-4");
    }

    #[test]
    fn test_fill_ratio() {
        let buffer = RingBuffer::new(100);

        let events: Vec<Event> = (0..50).map(|i| make_event(&format!("event-{i}"))).collect();
        buffer.push(events);

        assert!((buffer.fill_ratio() - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_metrics() {
        let buffer = RingBuffer::new(5);

        // Push 10 events (5 will be dropped)
        let events: Vec<Event> = (0..10).map(|i| make_event(&format!("event-{i}"))).collect();
        buffer.push(events);

        assert_eq!(buffer.total_dropped(), 5);

        // Drain all
        buffer.drain(5);
        assert_eq!(buffer.total_drained(), 5);
    }
}
