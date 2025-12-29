//! POLKU - Pluggable gRPC Event Gateway
//!
//! An open-source, format-agnostic event gateway that transforms and routes
//! events from any gRPC source to any destination.
//!
//! # Architecture
//!
//! ```text
//! Input Plugins ──► Core (buffer, route) ──► Output Plugins
//! ```
//!
//! Both inputs and outputs are pluggable via traits. Users provide their own
//! plugins for source-specific transformations and destination forwarding.

#![deny(unsafe_code)]
#![warn(clippy::unwrap_used)]
#![warn(clippy::expect_used)]
#![warn(clippy::panic)]

pub mod buffer;
pub mod config;
pub mod error;
pub mod hub;
pub mod input;
pub mod message;
pub mod metrics;
pub mod middleware;
pub mod output;
pub mod registry;
pub mod server;

// Proto types generated from polku/v1/gateway.proto
pub mod proto {
    #![allow(clippy::unwrap_used)]
    #![allow(clippy::expect_used)]
    #![allow(clippy::panic)]
    #![allow(clippy::derive_partial_eq_without_eq)]

    include!("proto/polku.v1.rs");

    // Re-export commonly used types at proto level for convenience
    pub use gateway_server::Gateway;
    pub use gateway_server::GatewayServer;
}

pub use config::Config;
pub use error::{PluginError, PolkuError, Result};
pub use hub::{Hub, HubRunner, MessageSender};
pub use input::{Input, InputContext};
pub use message::Message;
pub use middleware::{Filter, Middleware, MiddlewareChain, PassThrough, Transform};
pub use output::{Output, StdoutOutput};
pub use proto::Event;
pub use registry::PluginRegistry;
