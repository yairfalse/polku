//! Output system for POLKU
//!
//! Outputs send Events to various destinations (gRPC backends, Kafka, stdout, etc.)
//! All registered outputs receive events in a fan-out pattern.

pub mod stdout;

use crate::error::PluginError;
use crate::proto::Event;
use async_trait::async_trait;

pub use stdout::StdoutOutput;

/// Output trait - sends Events to destinations
///
/// Each output handles forwarding events to a specific destination.
/// Multiple outputs can be registered and events will be sent to all of them.
///
/// # Example
///
/// ```ignore
/// struct MyDestinationOutput {
///     client: MyGrpcClient,
/// }
///
/// #[async_trait]
/// impl Output for MyDestinationOutput {
///     fn name(&self) -> &'static str { "my-destination" }
///
///     async fn send(&self, events: &[Event]) -> Result<(), PluginError> {
///         self.client.send_events(events).await?;
///         Ok(())
///     }
///
///     async fn health(&self) -> bool {
///         self.client.ping().await.is_ok()
///     }
/// }
/// ```
#[async_trait]
pub trait Output: Send + Sync {
    /// Output name for identification and logging
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
    /// Called when the gateway is shutting down to flush buffers, close connections, etc.
    async fn shutdown(&self) -> Result<(), PluginError> {
        Ok(())
    }
}
