//! Stdout output for debugging
//!
//! Prints events to stdout in a human-readable format.
//! Useful for development and debugging.

use crate::error::PluginError;
use crate::output::Output;
use crate::proto::Event;
use async_trait::async_trait;
use std::sync::atomic::{AtomicU64, Ordering};

/// Stdout output - prints events for debugging
pub struct StdoutOutput {
    /// Pretty print events as JSON
    pretty: bool,
    /// Count of events sent
    sent_count: AtomicU64,
}

impl StdoutOutput {
    /// Create a new StdoutOutput plugin
    pub fn new() -> Self {
        Self {
            pretty: false,
            sent_count: AtomicU64::new(0),
        }
    }

    /// Create a new StdoutOutput plugin with pretty printing
    pub fn pretty() -> Self {
        Self {
            pretty: true,
            sent_count: AtomicU64::new(0),
        }
    }

    /// Get total events sent
    pub fn sent_count(&self) -> u64 {
        self.sent_count.load(Ordering::Relaxed)
    }
}

impl Default for StdoutOutput {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Output for StdoutOutput {
    fn name(&self) -> &'static str {
        "stdout"
    }

    async fn send(&self, events: &[Event]) -> Result<(), PluginError> {
        use std::io::Write;

        let mut stdout = std::io::stdout().lock();

        for event in events {
            if self.pretty {
                // Pretty format
                writeln!(
                    stdout,
                    "┌─ Event ─────────────────────────────────────────────",
                )
                .ok();
                writeln!(stdout, "│ ID:        {}", event.id).ok();
                writeln!(stdout, "│ Source:    {}", event.source).ok();
                writeln!(stdout, "│ Type:      {}", event.event_type).ok();
                writeln!(stdout, "│ Timestamp: {} ns", event.timestamp_unix_ns).ok();
                if !event.metadata.is_empty() {
                    writeln!(stdout, "│ Metadata:  {:?}", event.metadata).ok();
                }
                writeln!(stdout, "│ Payload:   {} bytes", event.payload.len()).ok();
                if !event.route_to.is_empty() {
                    writeln!(stdout, "│ Route to:  {:?}", event.route_to).ok();
                }
                writeln!(
                    stdout,
                    "└─────────────────────────────────────────────────────",
                )
                .ok();
            } else {
                // Compact format
                writeln!(
                    stdout,
                    "[{}] {}:{} ({} bytes)",
                    event.source,
                    event.event_type,
                    event.id,
                    event.payload.len()
                )
                .ok();
            }
        }

        self.sent_count
            .fetch_add(events.len() as u64, Ordering::Relaxed);

        Ok(())
    }

    async fn health(&self) -> bool {
        // Stdout is always healthy
        true
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn make_event(id: &str) -> Event {
        Event {
            id: id.to_string(),
            timestamp_unix_ns: 1234567890,
            source: "test-source".to_string(),
            event_type: "test".to_string(),
            metadata: Default::default(),
            payload: vec![1, 2, 3],
            route_to: vec![],
        }
    }

    #[tokio::test]
    async fn test_send_events() {
        let output = StdoutOutput::new();
        let events = vec![make_event("e1"), make_event("e2")];

        output.send(&events).await.unwrap();

        assert_eq!(output.sent_count(), 2);
    }

    #[tokio::test]
    async fn test_health() {
        let output = StdoutOutput::new();
        assert!(output.health().await);
    }
}
