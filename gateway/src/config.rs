//! Configuration for POLKU

use crate::error::{PolkuError, Result};
use std::env;
use std::net::SocketAddr;

/// Main configuration for POLKU
#[derive(Debug, Clone)]
pub struct Config {
    /// gRPC server address
    pub grpc_addr: SocketAddr,

    /// Metrics server address
    pub metrics_addr: SocketAddr,

    /// Buffer capacity (number of events)
    pub buffer_capacity: usize,

    /// Batch size for output plugins
    pub batch_size: usize,

    /// Flush interval in milliseconds
    pub flush_interval_ms: u64,

    /// Log level
    pub log_level: String,

    /// Log format (json or pretty)
    pub log_format: LogFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    Json,
    Pretty,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            grpc_addr: "0.0.0.0:50051".parse().unwrap(),
            metrics_addr: "0.0.0.0:9090".parse().unwrap(),
            buffer_capacity: 100_000,
            batch_size: 1000,
            flush_interval_ms: 100,
            log_level: "info".to_string(),
            log_format: LogFormat::Pretty,
        }
    }
}

impl Config {
    /// Load configuration from environment variables
    pub fn from_env() -> Result<Self> {
        let mut config = Config::default();

        if let Ok(addr) = env::var("POLKU_GRPC_ADDR") {
            config.grpc_addr = addr
                .parse()
                .map_err(|e| PolkuError::Config(format!("invalid POLKU_GRPC_ADDR: {e}")))?;
        }

        if let Ok(addr) = env::var("POLKU_METRICS_ADDR") {
            config.metrics_addr = addr
                .parse()
                .map_err(|e| PolkuError::Config(format!("invalid POLKU_METRICS_ADDR: {e}")))?;
        }

        if let Ok(cap) = env::var("POLKU_BUFFER_CAPACITY") {
            config.buffer_capacity = cap
                .parse()
                .map_err(|e| PolkuError::Config(format!("invalid POLKU_BUFFER_CAPACITY: {e}")))?;
        }

        if let Ok(size) = env::var("POLKU_BATCH_SIZE") {
            config.batch_size = size
                .parse()
                .map_err(|e| PolkuError::Config(format!("invalid POLKU_BATCH_SIZE: {e}")))?;
        }

        if let Ok(interval) = env::var("POLKU_FLUSH_INTERVAL_MS") {
            config.flush_interval_ms = interval
                .parse()
                .map_err(|e| PolkuError::Config(format!("invalid POLKU_FLUSH_INTERVAL_MS: {e}")))?;
        }

        if let Ok(level) = env::var("POLKU_LOG_LEVEL") {
            config.log_level = level;
        }

        if let Ok(format) = env::var("POLKU_LOG_FORMAT") {
            config.log_format = match format.to_lowercase().as_str() {
                "json" => LogFormat::Json,
                "pretty" => LogFormat::Pretty,
                other => {
                    return Err(PolkuError::Config(format!(
                        "invalid POLKU_LOG_FORMAT: {other} (expected 'json' or 'pretty')"
                    )))
                }
            };
        }

        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.buffer_capacity, 100_000);
        assert_eq!(config.batch_size, 1000);
        assert_eq!(config.flush_interval_ms, 100);
    }

    #[test]
    fn test_config_from_env() {
        // This test uses default values since env vars aren't set
        let config = Config::from_env().unwrap();
        assert!(config.buffer_capacity > 0);
    }
}
