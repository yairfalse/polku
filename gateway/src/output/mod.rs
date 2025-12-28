//! Output plugin system for POLKU
//!
//! Output plugins send Events to various destinations (AHTI, OTEL, stdout, file, etc.)

use crate::error::PluginError;
use crate::proto::Event;
use async_trait::async_trait;

/// Output plugin trait - sends Events to destinations
///
/// Each output plugin handles forwarding events to a specific destination.
#[async_trait]
pub trait OutputPlugin: Send + Sync {
    /// Plugin name for identification and logging
    fn name(&self) -> &'static str;

    /// Send events to the destination
    ///
    /// # Arguments
    /// * `events` - Slice of Events to send
    ///
    /// # Returns
    /// Ok(()) on success, PluginError on failure
    async fn send(&self, events: &[Event]) -> Result<(), PluginError>;

    /// Health check for the destination
    ///
    /// Returns true if the destination is healthy and accepting events.
    async fn health(&self) -> bool;

    /// Graceful shutdown
    ///
    /// Called when the gateway is shutting down to flush buffers, etc.
    async fn shutdown(&self) -> Result<(), PluginError> {
        Ok(())
    }
}
