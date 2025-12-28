//! Input plugin system for POLKU
//!
//! Input plugins transform raw bytes from various sources into unified Events.

use crate::error::PluginError;
use crate::proto::Event;
use async_trait::async_trait;

/// Input plugin trait - transforms raw bytes into Events
///
/// Each input plugin handles a specific source format (TAPIO, PORTTI, ELAVA, etc.)
/// and transforms it into the unified Event format.
#[async_trait]
pub trait InputPlugin: Send + Sync {
    /// Plugin name for identification and logging
    fn name(&self) -> &'static str;

    /// Transform raw bytes from a source into Events
    ///
    /// # Arguments
    /// * `source` - Source identifier (e.g., "tapio-node-1")
    /// * `data` - Raw bytes from the source
    ///
    /// # Returns
    /// Vector of Events or a PluginError
    fn transform(&self, source: &str, data: &[u8]) -> Result<Vec<Event>, PluginError>;
}
