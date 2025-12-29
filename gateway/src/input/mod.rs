//! Input system for POLKU
//!
//! Inputs transform raw bytes from various sources into unified Events.
//! Each source (e.g., "my-agent") registers an Input that knows how to
//! deserialize that source's format.

use crate::error::PluginError;
use crate::proto::Event;

/// Context for input transformation
#[derive(Debug, Clone)]
pub struct InputContext<'a> {
    /// Source identifier (e.g., "my-agent", "otel-collector")
    pub source: &'a str,
    /// Cluster/environment identifier
    pub cluster: &'a str,
    /// Format hint (e.g., "protobuf", "json", "msgpack")
    pub format: &'a str,
}

/// Input trait - transforms raw bytes into Events
///
/// Each input handles a specific source format and transforms it
/// into the unified Event format that POLKU uses internally.
///
/// # Example
///
/// ```ignore
/// struct MyAgentInput;
///
/// impl Input for MyAgentInput {
///     fn name(&self) -> &'static str { "my-agent" }
///
///     fn transform(&self, ctx: &InputContext, data: &[u8]) -> Result<Vec<Event>, PluginError> {
///         // Deserialize your format and create Events
///         let my_events: Vec<MyEvent> = deserialize(data)?;
///         Ok(my_events.into_iter().map(|e| e.into_event()).collect())
///     }
/// }
/// ```
pub trait Input: Send + Sync {
    /// Input name for identification and logging
    fn name(&self) -> &'static str;

    /// Transform raw bytes into Events
    ///
    /// # Arguments
    /// * `ctx` - Context with source, cluster, and format information
    /// * `data` - Raw bytes from the source
    ///
    /// # Returns
    /// Vector of Events or a PluginError
    fn transform(&self, ctx: &InputContext, data: &[u8]) -> Result<Vec<Event>, PluginError>;
}
