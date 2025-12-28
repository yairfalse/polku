//! POLKU - Pluggable gRPC Event Gateway
//!
//! A high-performance event gateway that transforms events from various sources
//! (TAPIO, PORTTI, ELAVA) into a unified format and forwards them to destinations
//! (AHTI, OTEL, etc.).
//!
//! # Architecture
//!
//! ```text
//! Input Plugins ──► Core (buffer, route) ──► Output Plugins
//! ```
//!
//! Both inputs and outputs are pluggable via traits.

#![deny(unsafe_code)]
#![warn(clippy::unwrap_used)]
#![warn(clippy::expect_used)]
#![warn(clippy::panic)]

pub mod buffer;
pub mod config;
pub mod error;
pub mod input;
pub mod metrics;
pub mod output;
pub mod server;

// Re-export proto types from central proto repo
pub mod proto {
    #![allow(clippy::unwrap_used)]
    #![allow(clippy::expect_used)]
    #![allow(clippy::panic)]
    #![allow(clippy::derive_partial_eq_without_eq)]

    /// AHTI event types (the unified event format)
    pub mod ahti {
        include!("proto/ahti.v1.rs");
    }

    /// POLKU gateway service types
    pub mod polku {
        include!("proto/polku.v1.rs");
    }

    // Re-export commonly used types at proto level for convenience
    pub use ahti::AhtiEvent as Event;
    pub use ahti::*;
    pub use polku::gateway_server;
    pub use polku::Ack;
    pub use polku::ComponentHealth;
    pub use polku::EventBatch;
    pub use polku::HealthRequest;
    pub use polku::HealthResponse;
}

pub use config::Config;
pub use error::{PolkuError, Result};
