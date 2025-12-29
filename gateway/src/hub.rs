//! Hub - the central pipeline builder for POLKU
//!
//! The Hub provides a builder pattern for configuring and running
//! the message pipeline. No YAML, just code.
//!
//! # Example
//!
//! ```ignore
//! use polku_gateway::{Hub, Message, Transform, StdoutOutput};
//!
//! Hub::new()
//!     .middleware(Transform::new(|mut msg| {
//!         msg.metadata.insert("processed".into(), "true".into());
//!         msg
//!     }))
//!     .output(StdoutOutput::new())
//!     .run()
//!     .await?;
//! ```

use crate::buffer::RingBuffer;
use crate::emit::Emitter;
use crate::error::PluginError;
use crate::message::Message;
use crate::middleware::{Middleware, MiddlewareChain};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

/// The Hub - central message pipeline
///
/// Connects inputs → middleware → buffer → outputs.
///
/// # Architecture
///
/// ```text
/// Input Channels ──► MiddlewareChain ──► RingBuffer ──► Outputs (fan-out)
/// ```
pub struct Hub {
    /// Buffer capacity
    buffer_capacity: usize,
    /// Middleware chain (applied before buffering)
    middleware: MiddlewareChain,
    /// Registered emitters
    emitters: Vec<Arc<dyn Emitter>>,
}

impl Hub {
    /// Create a new Hub with default settings
    pub fn new() -> Self {
        Self {
            buffer_capacity: 10_000,
            middleware: MiddlewareChain::new(),
            emitters: Vec::new(),
        }
    }

    /// Set the buffer capacity
    ///
    /// Default is 10,000 messages.
    pub fn buffer_capacity(mut self, capacity: usize) -> Self {
        self.buffer_capacity = capacity;
        self
    }

    /// Add a middleware to the processing chain
    ///
    /// Middleware is applied in order before messages enter the buffer.
    pub fn middleware<M: Middleware + 'static>(mut self, mw: M) -> Self {
        self.middleware.add(mw);
        self
    }

    /// Add an emitter destination
    ///
    /// All messages are sent to all emitters (fan-out).
    /// Use `route_to` field in Message to control routing.
    pub fn emitter<E: Emitter + 'static>(mut self, emitter: E) -> Self {
        self.emitters.push(Arc::new(emitter));
        self
    }

    /// Add an emitter destination (Arc version)
    pub fn emitter_arc(mut self, emitter: Arc<dyn Emitter>) -> Self {
        self.emitters.push(emitter);
        self
    }

    // Keep old name for compatibility
    pub fn output<E: Emitter + 'static>(self, emitter: E) -> Self {
        self.emitter(emitter)
    }

    /// Build a message sender for this hub
    ///
    /// Returns a sender that can be used to inject messages into the pipeline.
    /// This is useful for custom inputs or testing.
    pub fn build(self) -> (MessageSender, HubRunner) {
        let (tx, rx) = mpsc::channel(1024);

        let sender = MessageSender { tx };

        let runner = HubRunner {
            rx,
            buffer: Arc::new(RingBuffer::new(self.buffer_capacity)),
            middleware: self.middleware,
            emitters: self.emitters,
        };

        (sender, runner)
    }
}

impl Default for Hub {
    fn default() -> Self {
        Self::new()
    }
}

/// Message sender for injecting messages into the pipeline
#[derive(Clone)]
pub struct MessageSender {
    tx: mpsc::Sender<Message>,
}

impl MessageSender {
    /// Send a message into the pipeline
    pub async fn send(&self, msg: Message) -> Result<(), PluginError> {
        self.tx
            .send(msg)
            .await
            .map_err(|e| PluginError::Send(e.to_string()))
    }

    /// Try to send a message without blocking
    pub fn try_send(&self, msg: Message) -> Result<(), PluginError> {
        self.tx
            .try_send(msg)
            .map_err(|e| PluginError::Send(e.to_string()))
    }
}

/// Hub runner - processes messages through the pipeline
pub struct HubRunner {
    rx: mpsc::Receiver<Message>,
    buffer: Arc<RingBuffer>,
    middleware: MiddlewareChain,
    emitters: Vec<Arc<dyn Emitter>>,
}

impl HubRunner {
    /// Run the hub, processing messages until the channel closes
    ///
    /// This will:
    /// 1. Receive messages from the input channel
    /// 2. Apply middleware chain
    /// 3. Buffer messages
    /// 4. Periodically flush to outputs
    pub async fn run(mut self) -> Result<(), PluginError> {
        info!(
            emitters = self.emitters.len(),
            middleware = self.middleware.len(),
            buffer_capacity = self.buffer.capacity(),
            "Hub started"
        );

        if self.emitters.is_empty() {
            warn!("No emitters registered - messages will be buffered but not delivered");
        }

        // Spawn emitter flusher
        let buffer = Arc::clone(&self.buffer);
        let emitters = self.emitters.clone();
        let flush_handle = tokio::spawn(async move {
            flush_loop(buffer, emitters).await;
        });

        // Process incoming messages
        while let Some(msg) = self.rx.recv().await {
            // Apply middleware
            let processed = self.middleware.process(msg).await;

            if let Some(msg) = processed {
                debug!(id = %msg.id, "Message buffered");
                let dropped = self.buffer.push(vec![msg]);
                if dropped > 0 {
                    warn!(dropped = dropped, "Buffer overflow, messages dropped");
                }
            }
        }

        // Channel closed, wait for flush to complete
        flush_handle.abort();
        info!("Hub shutdown");

        Ok(())
    }

    /// Get a reference to the buffer for monitoring
    pub fn buffer(&self) -> &Arc<RingBuffer> {
        &self.buffer
    }
}

/// Background flush loop - sends buffered messages to emitters
async fn flush_loop(buffer: Arc<RingBuffer>, emitters: Vec<Arc<dyn Emitter>>) {
    const BATCH_SIZE: usize = 100;
    const FLUSH_INTERVAL_MS: u64 = 10;

    loop {
        tokio::time::sleep(tokio::time::Duration::from_millis(FLUSH_INTERVAL_MS)).await;

        let messages = buffer.drain(BATCH_SIZE);
        if messages.is_empty() {
            continue;
        }

        // Convert Messages to proto Events for outputs
        let events: Vec<crate::proto::Event> = messages
            .into_iter()
            .map(crate::proto::Event::from)
            .collect();

        // Send to all emitters (fan-out)
        for emitter in &emitters {
            // Check routing
            let routed_events: Vec<_> = events
                .iter()
                .filter(|e| e.route_to.is_empty() || e.route_to.iter().any(|r| r == emitter.name()))
                .cloned()
                .collect();

            if routed_events.is_empty() {
                continue;
            }

            if let Err(e) = emitter.emit(&routed_events).await {
                error!(
                    emitter = emitter.name(),
                    error = %e,
                    count = routed_events.len(),
                    "Failed to emit"
                );
            } else {
                debug!(
                    emitter = emitter.name(),
                    count = routed_events.len(),
                    "Emitted"
                );
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::emit::StdoutEmitter;
    use crate::middleware::{Filter, Transform};
    use bytes::Bytes;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_hub_builder() {
        let hub = Hub::new()
            .buffer_capacity(1000)
            .middleware(Transform::new(|msg| msg))
            .emitter(StdoutEmitter::new());

        assert_eq!(hub.buffer_capacity, 1000);
        assert_eq!(hub.emitters.len(), 1);
    }

    #[test]
    fn test_hub_build() {
        let hub = Hub::new().emitter(StdoutEmitter::new());

        let (sender, runner) = hub.build();

        // Sender should be cloneable
        let _sender2 = sender.clone();

        // Runner should have buffer
        assert_eq!(runner.buffer.capacity(), 10_000);
    }

    #[tokio::test]
    async fn test_message_sender() {
        let hub = Hub::new();
        let (sender, _runner) = hub.build();

        let msg = Message::new("test", "evt", Bytes::from("payload"));
        sender.send(msg).await.expect("should send");
    }

    #[tokio::test]
    async fn test_hub_with_middleware() {
        // Track how many messages pass through
        static COUNTER: AtomicUsize = AtomicUsize::new(0);

        struct CountingMiddleware;

        #[async_trait::async_trait]
        impl Middleware for CountingMiddleware {
            fn name(&self) -> &'static str {
                "counter"
            }

            async fn process(&self, msg: Message) -> Option<Message> {
                COUNTER.fetch_add(1, Ordering::Relaxed);
                Some(msg)
            }
        }

        let hub = Hub::new().middleware(CountingMiddleware);

        let (sender, runner) = hub.build();

        // Send messages in background
        let sender_handle = tokio::spawn(async move {
            for i in 0..5 {
                let msg = Message::new("test", format!("evt-{i}"), Bytes::new());
                sender.send(msg).await.ok();
            }
            // Drop sender to close channel
        });

        // Run hub briefly
        let runner_handle = tokio::spawn(async move {
            tokio::time::timeout(tokio::time::Duration::from_millis(100), runner.run())
                .await
                .ok();
        });

        sender_handle.await.ok();
        runner_handle.await.ok();

        // All 5 messages should have been processed
        assert_eq!(COUNTER.load(Ordering::Relaxed), 5);
    }

    #[tokio::test]
    async fn test_hub_filter() {
        let hub = Hub::new()
            // Only allow messages with type "keep"
            .middleware(Filter::new(|msg: &Message| msg.message_type == "keep"));

        let (sender, runner) = hub.build();
        let buffer = Arc::clone(runner.buffer());

        // Send messages
        let sender_handle = tokio::spawn(async move {
            sender
                .send(Message::new("test", "keep", Bytes::new()))
                .await
                .ok();
            sender
                .send(Message::new("test", "drop", Bytes::new()))
                .await
                .ok();
            sender
                .send(Message::new("test", "keep", Bytes::new()))
                .await
                .ok();
        });

        // Run briefly
        let runner_handle = tokio::spawn(async move {
            tokio::time::timeout(tokio::time::Duration::from_millis(50), runner.run())
                .await
                .ok();
        });

        sender_handle.await.ok();
        runner_handle.await.ok();

        // Only 2 messages should be in buffer (the "keep" ones)
        assert_eq!(buffer.len(), 2);
    }
}
