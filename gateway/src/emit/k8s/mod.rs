//! Kubernetes emitter integration via Seppo
//!
//! Emit POLKU events as Kubernetes resources (ConfigMaps, Custom Resources).
//!
//! # Feature Flag
//!
//! This module requires the `k8s` feature:
//!
//! ```toml
//! polku-gateway = { version = "0.1", features = ["k8s"] }
//! ```
//!
//! # Example
//!
//! ```ignore
//! use polku_gateway::{Hub, K8sEmitter};
//!
//! // Option 1: Use default kubeconfig
//! let emitter = K8sEmitter::new().await?;
//!
//! // Option 2: Use Seppo context
//! // let ctx = seppo::Context::new().await?;
//! // let emitter = K8sEmitter::from_seppo(&ctx)?;
//!
//! Hub::new()
//!     .emitter(emitter)
//!     .run()
//!     .await?;
//! ```

mod emitter;

pub use emitter::{K8sEmitter, K8sEmitterConfig, ResourceKind};
