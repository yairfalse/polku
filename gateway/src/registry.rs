//! Plugin registry for POLKU
//!
//! Manages inputs and outputs. Inputs are keyed by source name for O(1) lookup
//! during transformation. Outputs are stored in a vector for fan-out delivery.

use crate::error::PluginError;
use crate::input::{Input, InputContext};
use crate::output::Output;
use crate::proto::Event;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

/// Registry for inputs and outputs
///
/// Thread-safe container for managing inputs and outputs. Typically populated
/// at startup and then used read-only during operation.
pub struct PluginRegistry {
    /// Inputs keyed by source name
    inputs: HashMap<String, Arc<dyn Input>>,
    /// Outputs (fan-out to all)
    outputs: Vec<Arc<dyn Output>>,
}

impl PluginRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            inputs: HashMap::new(),
            outputs: Vec::new(),
        }
    }

    /// Register an input for a source
    ///
    /// The source name is used to route incoming raw events to the right transformer.
    pub fn register_input(&mut self, source: impl Into<String>, input: Arc<dyn Input>) {
        let source = source.into();
        info!(source = %source, input = input.name(), "Registered input");
        self.inputs.insert(source, input);
    }

    /// Register an output
    ///
    /// All events will be sent to all registered outputs.
    pub fn register_output(&mut self, output: Arc<dyn Output>) {
        info!(output = output.name(), "Registered output");
        self.outputs.push(output);
    }

    /// Check if an input is registered for a source
    pub fn has_input(&self, source: &str) -> bool {
        self.inputs.contains_key(source)
    }

    /// Get the number of registered inputs
    pub fn input_count(&self) -> usize {
        self.inputs.len()
    }

    /// Get the number of registered outputs
    pub fn output_count(&self) -> usize {
        self.outputs.len()
    }

    /// Transform raw bytes using the appropriate input
    ///
    /// Looks up the input by source name and calls its transform method.
    /// Returns an error if no input is registered for the source.
    pub fn transform(&self, ctx: &InputContext, data: &[u8]) -> Result<Vec<Event>, PluginError> {
        let input = self.inputs.get(ctx.source).ok_or_else(|| {
            PluginError::Transform(format!("No input registered for source '{}'", ctx.source))
        })?;

        debug!(
            source = %ctx.source,
            input = input.name(),
            bytes = data.len(),
            "Transforming raw data"
        );

        input.transform(ctx, data)
    }

    /// Send events to all outputs
    ///
    /// Events are sent to each output. Failures are logged but don't
    /// stop delivery to other outputs.
    ///
    /// Returns the number of successful deliveries.
    pub async fn send_to_outputs(&self, events: &[Event]) -> usize {
        if self.outputs.is_empty() {
            warn!("No outputs registered, events will be dropped");
            return 0;
        }

        let mut success_count = 0;

        for output in &self.outputs {
            match output.send(events).await {
                Ok(()) => {
                    debug!(plugin = output.name(), count = events.len(), "Events sent");
                    success_count += 1;
                }
                Err(e) => {
                    error!(
                        plugin = output.name(),
                        error = %e,
                        count = events.len(),
                        "Failed to send events"
                    );
                }
            }
        }

        success_count
    }

    /// Send events with routing hints
    ///
    /// Events are sent only to outputs whose names match the route_to field.
    /// If route_to is empty, sends to all outputs.
    pub async fn send_with_routing(&self, events: &[Event]) -> usize {
        // Group events by their routing
        let mut all_outputs: Vec<&Event> = Vec::new();
        let mut routed: HashMap<&str, Vec<&Event>> = HashMap::new();

        for event in events {
            if event.route_to.is_empty() {
                all_outputs.push(event);
            } else {
                for route in &event.route_to {
                    routed.entry(route.as_str()).or_default().push(event);
                }
            }
        }

        let mut success_count = 0;

        // Send events without routing to all outputs
        if !all_outputs.is_empty() {
            let events_vec: Vec<Event> = all_outputs.into_iter().cloned().collect();
            success_count += self.send_to_outputs(&events_vec).await;
        }

        // Send routed events to specific outputs
        for output in &self.outputs {
            if let Some(events) = routed.get(output.name()) {
                let events_vec: Vec<Event> = events.iter().copied().cloned().collect();
                match output.send(&events_vec).await {
                    Ok(()) => {
                        debug!(
                            plugin = output.name(),
                            count = events_vec.len(),
                            "Routed events sent"
                        );
                        success_count += 1;
                    }
                    Err(e) => {
                        error!(
                            plugin = output.name(),
                            error = %e,
                            count = events_vec.len(),
                            "Failed to send routed events"
                        );
                    }
                }
            }
        }

        success_count
    }

    /// Check health of all outputs
    ///
    /// Returns a map of output name to health status.
    pub async fn output_health(&self) -> HashMap<String, bool> {
        let mut health = HashMap::new();

        for output in &self.outputs {
            let is_healthy = output.health().await;
            health.insert(output.name().to_string(), is_healthy);
        }

        health
    }

    /// Graceful shutdown of all outputs
    pub async fn shutdown(&self) -> Result<(), PluginError> {
        info!("Shutting down {} outputs", self.outputs.len());

        for output in &self.outputs {
            if let Err(e) = output.shutdown().await {
                error!(output = output.name(), error = %e, "Error during output shutdown");
            }
        }

        Ok(())
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    struct MockInput {
        name: &'static str,
    }

    impl Input for MockInput {
        fn name(&self) -> &'static str {
            self.name
        }

        fn transform(&self, ctx: &InputContext, data: &[u8]) -> Result<Vec<Event>, PluginError> {
            Ok(vec![Event {
                id: format!("{}:{}", ctx.source, data.len()),
                timestamp_unix_ns: 0,
                source: ctx.source.to_string(),
                event_type: "test".to_string(),
                metadata: Default::default(),
                payload: data.to_vec(),
                route_to: vec![],
            }])
        }
    }

    struct MockOutput {
        name: &'static str,
    }

    #[async_trait::async_trait]
    impl Output for MockOutput {
        fn name(&self) -> &'static str {
            self.name
        }

        async fn send(&self, _events: &[Event]) -> Result<(), PluginError> {
            Ok(())
        }

        async fn health(&self) -> bool {
            true
        }
    }

    #[test]
    fn test_register_input() {
        let mut registry = PluginRegistry::new();
        let plugin = Arc::new(MockInput { name: "test-input" });

        registry.register_input("test-source", plugin);

        assert!(registry.has_input("test-source"));
        assert!(!registry.has_input("unknown"));
        assert_eq!(registry.input_count(), 1);
    }

    #[test]
    fn test_register_output() {
        let mut registry = PluginRegistry::new();
        let plugin = Arc::new(MockOutput {
            name: "test-output",
        });

        registry.register_output(plugin);

        assert_eq!(registry.output_count(), 1);
    }

    #[test]
    fn test_transform() {
        let mut registry = PluginRegistry::new();
        let plugin = Arc::new(MockInput { name: "test-input" });
        registry.register_input("test-source", plugin);

        let ctx = InputContext {
            source: "test-source",
            cluster: "test-cluster",
            format: "protobuf",
        };

        let events = registry.transform(&ctx, b"hello").unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].id, "test-source:5");
    }

    #[test]
    fn test_transform_unknown_source() {
        let registry = PluginRegistry::new();

        let ctx = InputContext {
            source: "unknown",
            cluster: "test-cluster",
            format: "protobuf",
        };

        let result = registry.transform(&ctx, b"hello");
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_send_to_outputs() {
        let mut registry = PluginRegistry::new();
        registry.register_output(Arc::new(MockOutput { name: "output1" }));
        registry.register_output(Arc::new(MockOutput { name: "output2" }));

        let events = vec![Event {
            id: "test".to_string(),
            timestamp_unix_ns: 0,
            source: "test".to_string(),
            event_type: "test".to_string(),
            metadata: Default::default(),
            payload: vec![],
            route_to: vec![],
        }];

        let success = registry.send_to_outputs(&events).await;
        assert_eq!(success, 2);
    }

    #[tokio::test]
    async fn test_output_health() {
        let mut registry = PluginRegistry::new();
        registry.register_output(Arc::new(MockOutput { name: "output1" }));

        let health = registry.output_health().await;
        assert_eq!(health.get("output1"), Some(&true));
    }
}
