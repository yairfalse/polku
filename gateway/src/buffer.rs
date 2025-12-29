//! Ring buffer for message buffering with backpressure support

use crate::message::Message;
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};

/// Thread-safe ring buffer for messages
///
/// When full, oldest messages are dropped (FIFO eviction).
/// Provides metrics for monitoring buffer state.
pub struct RingBuffer {
    messages: Mutex<VecDeque<Message>>,
    capacity: usize,
    metrics: BufferMetrics,
}

/// Metrics for buffer monitoring
pub struct BufferMetrics {
    /// Total messages pushed
    pub pushed: AtomicU64,
    /// Total messages dropped due to full buffer
    pub dropped: AtomicU64,
    /// Total messages drained
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
            messages: Mutex::new(VecDeque::with_capacity(capacity)),
            capacity,
            metrics: BufferMetrics::default(),
        }
    }

    /// Push messages into the buffer
    ///
    /// Returns the number of messages dropped due to capacity limits.
    pub fn push(&self, messages: Vec<Message>) -> usize {
        let mut buffer = self.messages.lock();
        let mut dropped = 0;

        for msg in messages {
            if buffer.len() >= self.capacity {
                // Drop oldest message (FIFO eviction)
                buffer.pop_front();
                dropped += 1;
            }
            buffer.push_back(msg);
        }

        self.metrics
            .pushed
            .fetch_add((buffer.len() + dropped) as u64, Ordering::Relaxed);
        self.metrics
            .dropped
            .fetch_add(dropped as u64, Ordering::Relaxed);

        dropped
    }

    /// Drain up to `n` messages from the buffer
    ///
    /// Returns the drained messages in FIFO order.
    pub fn drain(&self, n: usize) -> Vec<Message> {
        let mut buffer = self.messages.lock();
        let drain_count = n.min(buffer.len());
        let messages: Vec<Message> = buffer.drain(..drain_count).collect();

        self.metrics
            .drained
            .fetch_add(messages.len() as u64, Ordering::Relaxed);

        messages
    }

    /// Get current number of messages in buffer
    pub fn len(&self) -> usize {
        self.messages.lock().len()
    }

    /// Check if buffer is empty
    pub fn is_empty(&self) -> bool {
        self.messages.lock().is_empty()
    }

    /// Check if buffer is at capacity
    pub fn is_full(&self) -> bool {
        self.messages.lock().len() >= self.capacity
    }

    /// Get buffer capacity
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Get current fill percentage (0.0 - 1.0)
    pub fn fill_ratio(&self) -> f64 {
        let len = self.messages.lock().len();
        len as f64 / self.capacity as f64
    }

    /// Get total messages pushed
    pub fn total_pushed(&self) -> u64 {
        self.metrics.pushed.load(Ordering::Relaxed)
    }

    /// Get total messages dropped
    pub fn total_dropped(&self) -> u64 {
        self.metrics.dropped.load(Ordering::Relaxed)
    }

    /// Get total messages drained
    pub fn total_drained(&self) -> u64 {
        self.metrics.drained.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;

    fn make_message(id: &str) -> Message {
        Message::with_id(id, 0, "test", "test", Bytes::new())
    }

    #[test]
    fn test_push_and_drain() {
        let buffer = RingBuffer::new(10);

        // Push 5 messages
        let messages: Vec<Message> = (0..5).map(|i| make_message(&format!("msg-{i}"))).collect();
        let dropped = buffer.push(messages);

        assert_eq!(dropped, 0);
        assert_eq!(buffer.len(), 5);

        // Drain 3 messages
        let drained = buffer.drain(3);
        assert_eq!(drained.len(), 3);
        assert_eq!(drained[0].id, "msg-0");
        assert_eq!(drained[2].id, "msg-2");
        assert_eq!(buffer.len(), 2);
    }

    #[test]
    fn test_overflow_drops_oldest() {
        let buffer = RingBuffer::new(3);

        // Push 5 messages into a buffer of size 3
        let messages: Vec<Message> = (0..5).map(|i| make_message(&format!("msg-{i}"))).collect();
        let dropped = buffer.push(messages);

        assert_eq!(dropped, 2); // 2 messages dropped
        assert_eq!(buffer.len(), 3);

        // Should have messages 2, 3, 4 (oldest 0, 1 dropped)
        let drained = buffer.drain(3);
        assert_eq!(drained[0].id, "msg-2");
        assert_eq!(drained[1].id, "msg-3");
        assert_eq!(drained[2].id, "msg-4");
    }

    #[test]
    fn test_fill_ratio() {
        let buffer = RingBuffer::new(100);

        let messages: Vec<Message> = (0..50).map(|i| make_message(&format!("msg-{i}"))).collect();
        buffer.push(messages);

        assert!((buffer.fill_ratio() - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_metrics() {
        let buffer = RingBuffer::new(5);

        // Push 10 messages (5 will be dropped)
        let messages: Vec<Message> = (0..10).map(|i| make_message(&format!("msg-{i}"))).collect();
        buffer.push(messages);

        assert_eq!(buffer.total_dropped(), 5);

        // Drain all
        buffer.drain(5);
        assert_eq!(buffer.total_drained(), 5);
    }
}
