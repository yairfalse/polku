//! HTTP server for Prometheus metrics endpoint
//!
//! Runs a lightweight HTTP server on a separate port for Prometheus scraping.
//!
//! # Example
//!
//! ```ignore
//! use polku_gateway::metrics_server::MetricsServer;
//!
//! // Start metrics server on port 9090
//! let metrics_handle = MetricsServer::start(9090);
//!
//! // Later, to shutdown
//! metrics_handle.abort();
//! ```

use axum::{Router, http::StatusCode, response::IntoResponse, routing::get};
use std::net::SocketAddr;
use tokio::task::JoinHandle;
use tracing::{error, info};

/// Metrics HTTP server
pub struct MetricsServer;

impl MetricsServer {
    /// Start the metrics server on the given port
    ///
    /// Returns a JoinHandle that can be used to abort the server.
    /// The server runs until aborted or the process exits.
    pub fn start(port: u16) -> JoinHandle<()> {
        let addr = SocketAddr::from(([0, 0, 0, 0], port));

        tokio::spawn(async move {
            let app = Router::new()
                .route("/metrics", get(metrics_handler))
                .route("/health", get(health_handler));

            info!(port = port, "Metrics server starting");

            let listener = match tokio::net::TcpListener::bind(addr).await {
                Ok(l) => l,
                Err(e) => {
                    error!(error = %e, port = port, "Failed to bind metrics server");
                    return;
                }
            };

            if let Err(e) = axum::serve(listener, app).await {
                error!(error = %e, "Metrics server error");
            }
        })
    }
}

/// Handler for /metrics endpoint
async fn metrics_handler() -> impl IntoResponse {
    let body = crate::metrics::gather();
    (
        StatusCode::OK,
        [("content-type", "text/plain; version=0.0.4; charset=utf-8")],
        body,
    )
}

/// Handler for /health endpoint
async fn health_handler() -> impl IntoResponse {
    (StatusCode::OK, "OK")
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_metrics_handler_returns_prometheus_format() {
        // Initialize metrics first
        let _ = crate::metrics::Metrics::init();

        let response = metrics_handler().await.into_response();
        assert_eq!(response.status(), StatusCode::OK);

        // Check content type
        let content_type = response
            .headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap();
        assert!(content_type.contains("text/plain"));
    }

    #[tokio::test]
    async fn test_health_handler() {
        let response = health_handler().await.into_response();
        assert_eq!(response.status(), StatusCode::OK);
    }
}
