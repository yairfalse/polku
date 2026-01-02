//! Per-source throttle middleware
//!
//! Like RateLimiter but maintains separate limits per source.
//! Useful when different sources have different throughput needs.

use crate::message::Message;
use crate::middleware::Middleware;
use async_trait::async_trait;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// Per-source rate limiter
///
/// Each source gets its own token bucket with the configured rate/burst.
/// New sources are allocated a bucket on first message.
pub struct Throttle {
    /// Rate per second for each source
    rate: u64,
    /// Burst capacity for each source
    burst: u64,
    /// Per-source buckets
    buckets: RwLock<HashMap<String, TokenBucket>>,
}

/// Individual token bucket (similar to RateLimiter internals)
struct TokenBucket {
    capacity: u64,
    refill_amount: u64,
    refill_nanos: u64,
    tokens: AtomicU64,
    last_refill: AtomicU64,
    start: Instant,
}

impl TokenBucket {
    fn new(rate: u64, burst: u64) -> Self {
        let refill_nanos = if rate == 0 {
            u64::MAX
        } else {
            1_000_000_000 / rate
        };

        let scaled_burst = burst.saturating_mul(1000);

        Self {
            capacity: scaled_burst,
            refill_amount: 1000,
            refill_nanos,
            tokens: AtomicU64::new(scaled_burst),
            last_refill: AtomicU64::new(0),
            start: Instant::now(),
        }
    }

    fn try_acquire(&self) -> bool {
        self.refill();

        loop {
            let current = self.tokens.load(Ordering::Acquire);
            if current < 1000 {
                return false;
            }

            if self
                .tokens
                .compare_exchange_weak(current, current - 1000, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return true;
            }
        }
    }

    fn refill(&self) {
        let now_nanos = self.start.elapsed().as_nanos() as u64;

        loop {
            let last = self.last_refill.load(Ordering::Acquire);
            let elapsed = now_nanos.saturating_sub(last);

            if elapsed < self.refill_nanos {
                return;
            }

            let intervals = elapsed / self.refill_nanos;
            if intervals == 0 {
                return;
            }

            let new_last = last + intervals * self.refill_nanos;

            match self.last_refill.compare_exchange_weak(
                last,
                new_last,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    let tokens_to_add = intervals * self.refill_amount;
                    if tokens_to_add == 0 {
                        return;
                    }

                    loop {
                        let current = self.tokens.load(Ordering::Acquire);
                        let new_tokens = (current.saturating_add(tokens_to_add)).min(self.capacity);
                        if current == new_tokens {
                            break;
                        }
                        if self
                            .tokens
                            .compare_exchange_weak(
                                current,
                                new_tokens,
                                Ordering::AcqRel,
                                Ordering::Acquire,
                            )
                            .is_ok()
                        {
                            break;
                        }
                    }
                    return;
                }
                Err(_) => continue,
            }
        }
    }
}

impl Throttle {
    /// Create a new per-source throttle
    ///
    /// Each source gets its own bucket with the specified rate and burst.
    pub fn new(rate: u64, burst: u64) -> Self {
        Self {
            rate,
            burst,
            buckets: RwLock::new(HashMap::new()),
        }
    }

    /// Get or create a bucket for the given source
    fn get_or_create_bucket(&self, source: &str) -> bool {
        // Fast path: check if bucket exists
        {
            let buckets = self.buckets.read();
            if let Some(bucket) = buckets.get(source) {
                return bucket.try_acquire();
            }
        }

        // Slow path: create new bucket
        let mut buckets = self.buckets.write();
        // Double-check after acquiring write lock
        if let Some(bucket) = buckets.get(source) {
            return bucket.try_acquire();
        }

        let bucket = TokenBucket::new(self.rate, self.burst);
        let result = bucket.try_acquire();
        buckets.insert(source.to_string(), bucket);
        result
    }

    /// Get the number of tracked sources
    pub fn source_count(&self) -> usize {
        self.buckets.read().len()
    }
}

#[async_trait]
impl Middleware for Throttle {
    fn name(&self) -> &'static str {
        "throttle"
    }

    async fn process(&self, msg: Message) -> Option<Message> {
        if self.get_or_create_bucket(&msg.source) {
            Some(msg)
        } else {
            tracing::debug!(
                source = %msg.source,
                message_type = %msg.message_type,
                "throttled"
            );
            None
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use bytes::Bytes;

    #[tokio::test]
    async fn test_throttle_allows_within_limit() {
        let throttle = Throttle::new(100, 10);
        let msg = Message::new("source-a", "evt", Bytes::new());

        let result = throttle.process(msg).await;
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn test_throttle_blocks_over_burst() {
        let throttle = Throttle::new(100, 2); // 100/s, burst 2

        // Consume burst for source-a
        for _ in 0..2 {
            let msg = Message::new("source-a", "evt", Bytes::new());
            assert!(throttle.process(msg).await.is_some());
        }

        // Third from source-a should be blocked
        let msg = Message::new("source-a", "evt", Bytes::new());
        assert!(throttle.process(msg).await.is_none());
    }

    #[tokio::test]
    async fn test_throttle_independent_per_source() {
        let throttle = Throttle::new(100, 2); // 100/s, burst 2

        // Exhaust source-a
        for _ in 0..2 {
            let msg = Message::new("source-a", "evt", Bytes::new());
            assert!(throttle.process(msg).await.is_some());
        }
        let msg = Message::new("source-a", "evt", Bytes::new());
        assert!(throttle.process(msg).await.is_none());

        // source-b should still work (separate bucket)
        let msg = Message::new("source-b", "evt", Bytes::new());
        assert!(throttle.process(msg).await.is_some());

        // Verify we have 2 sources tracked
        assert_eq!(throttle.source_count(), 2);
    }

    #[tokio::test]
    async fn test_throttle_refills() {
        let throttle = Throttle::new(1000, 1); // 1000/s, burst 1

        // Consume the burst
        let msg = Message::new("source-a", "evt", Bytes::new());
        assert!(throttle.process(msg).await.is_some());

        // Immediately blocked
        let msg = Message::new("source-a", "evt", Bytes::new());
        assert!(throttle.process(msg).await.is_none());

        // Wait for refill
        tokio::time::sleep(tokio::time::Duration::from_millis(2)).await;

        // Should be allowed again
        let msg = Message::new("source-a", "evt", Bytes::new());
        assert!(throttle.process(msg).await.is_some());
    }

    #[tokio::test]
    async fn test_throttle_concurrent_sources() {
        use std::sync::Arc;

        let throttle = Arc::new(Throttle::new(100, 5));
        let mut handles = vec![];

        // 10 concurrent sources
        for i in 0..10 {
            let throttle = Arc::clone(&throttle);
            handles.push(tokio::spawn(async move {
                let source = format!("source-{}", i);
                let mut passed = 0;
                for _ in 0..10 {
                    let msg = Message::new(&source, "evt", Bytes::new());
                    if throttle.process(msg).await.is_some() {
                        passed += 1;
                    }
                }
                passed
            }));
        }

        let results: Vec<_> = futures::future::join_all(handles)
            .await
            .into_iter()
            .map(|r| r.unwrap())
            .collect();

        // Each source should pass exactly 5 (their burst)
        for (i, passed) in results.iter().enumerate() {
            assert_eq!(*passed, 5, "source-{} passed {} instead of 5", i, passed);
        }

        // All 10 sources should be tracked
        assert_eq!(throttle.source_count(), 10);
    }
}
