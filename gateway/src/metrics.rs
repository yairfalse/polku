//! Prometheus metrics for POLKU

use prometheus::{
    register_counter_vec, register_gauge, register_histogram_vec, CounterVec, Gauge, HistogramVec,
};
use std::sync::OnceLock;

/// Global metrics instance
static METRICS: OnceLock<Metrics> = OnceLock::new();

/// All POLKU metrics
pub struct Metrics {
    /// Events received (by source, type)
    pub events_received: CounterVec,

    /// Events forwarded (by output, type)
    pub events_forwarded: CounterVec,

    /// Events dropped (by reason)
    pub events_dropped: CounterVec,

    /// Current buffer size
    pub buffer_size: Gauge,

    /// Buffer capacity
    pub buffer_capacity: Gauge,

    /// Event processing latency (by source)
    pub processing_latency: HistogramVec,

    /// Active streams (by source)
    pub active_streams: Gauge,

    /// Plugin health (1 = healthy, 0 = unhealthy)
    pub plugin_health: Gauge,
}

impl Metrics {
    /// Initialize metrics (call once at startup)
    pub fn init() -> &'static Metrics {
        METRICS.get_or_init(|| {
            Metrics {
                events_received: register_counter_vec!(
                    "polku_events_received_total",
                    "Total events received",
                    &["source", "type"]
                )
                .unwrap_or_else(|e| panic!("Failed to register events_received metric: {e}")),

                events_forwarded: register_counter_vec!(
                    "polku_events_forwarded_total",
                    "Total events forwarded to outputs",
                    &["output", "type"]
                )
                .unwrap_or_else(|e| panic!("Failed to register events_forwarded metric: {e}")),

                events_dropped: register_counter_vec!(
                    "polku_events_dropped_total",
                    "Total events dropped",
                    &["reason"]
                )
                .unwrap_or_else(|e| panic!("Failed to register events_dropped metric: {e}")),

                buffer_size: register_gauge!(
                    "polku_buffer_size",
                    "Current number of events in buffer"
                )
                .unwrap_or_else(|e| panic!("Failed to register buffer_size metric: {e}")),

                buffer_capacity: register_gauge!(
                    "polku_buffer_capacity",
                    "Maximum buffer capacity"
                )
                .unwrap_or_else(|e| panic!("Failed to register buffer_capacity metric: {e}")),

                processing_latency: register_histogram_vec!(
                    "polku_processing_latency_seconds",
                    "Event processing latency",
                    &["source"],
                    vec![0.0001, 0.0005, 0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0]
                )
                .unwrap_or_else(|e| panic!("Failed to register processing_latency metric: {e}")),

                active_streams: register_gauge!(
                    "polku_active_streams",
                    "Number of active gRPC streams"
                )
                .unwrap_or_else(|e| panic!("Failed to register active_streams metric: {e}")),

                plugin_health: register_gauge!(
                    "polku_plugin_health",
                    "Plugin health status (1 = healthy, 0 = unhealthy)"
                )
                .unwrap_or_else(|e| panic!("Failed to register plugin_health metric: {e}")),
            }
        })
    }

    /// Get the global metrics instance
    pub fn get() -> &'static Metrics {
        METRICS.get().expect("Metrics not initialized - call Metrics::init() first")
    }

    /// Record event received
    pub fn record_received(&self, source: &str, event_type: &str, count: u64) {
        self.events_received
            .with_label_values(&[source, event_type])
            .inc_by(count as f64);
    }

    /// Record event forwarded
    pub fn record_forwarded(&self, output: &str, event_type: &str, count: u64) {
        self.events_forwarded
            .with_label_values(&[output, event_type])
            .inc_by(count as f64);
    }

    /// Record events dropped
    pub fn record_dropped(&self, reason: &str, count: u64) {
        self.events_dropped
            .with_label_values(&[reason])
            .inc_by(count as f64);
    }

    /// Update buffer size
    pub fn set_buffer_size(&self, size: usize) {
        self.buffer_size.set(size as f64);
    }

    /// Update buffer capacity
    pub fn set_buffer_capacity(&self, capacity: usize) {
        self.buffer_capacity.set(capacity as f64);
    }

    /// Record processing latency
    pub fn record_latency(&self, source: &str, seconds: f64) {
        self.processing_latency
            .with_label_values(&[source])
            .observe(seconds);
    }

    /// Increment active streams
    pub fn inc_streams(&self) {
        self.active_streams.inc();
    }

    /// Decrement active streams
    pub fn dec_streams(&self) {
        self.active_streams.dec();
    }

    /// Set plugin health
    pub fn set_plugin_health(&self, healthy: bool) {
        self.plugin_health.set(if healthy { 1.0 } else { 0.0 });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_init() {
        let metrics = Metrics::init();
        metrics.record_received("tapio", "network", 10);
        metrics.set_buffer_size(100);
    }
}
