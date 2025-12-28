//! gRPC server for POLKU Gateway
//!
//! Implements the Gateway service defined in proto/v1/gateway.proto

use crate::buffer::RingBuffer;
use crate::proto::{
    gateway_server::{Gateway, GatewayServer},
    Ack, Event, EventBatch, HealthRequest, HealthResponse,
};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status, Streaming};
use tracing::info;

/// Gateway gRPC service implementation
pub struct GatewayService {
    buffer: Arc<RingBuffer>,
    start_time: Instant,
    events_processed: AtomicU64,
}

impl GatewayService {
    /// Create a new GatewayService
    pub fn new(buffer: Arc<RingBuffer>) -> Self {
        Self {
            buffer,
            start_time: Instant::now(),
            events_processed: AtomicU64::new(0),
        }
    }

    /// Create a tonic server from this service
    pub fn into_server(self) -> GatewayServer<Self> {
        GatewayServer::new(self)
    }
}

#[tonic::async_trait]
impl Gateway for GatewayService {
    type StreamEventsStream = ReceiverStream<Result<Ack, Status>>;

    async fn stream_events(
        &self,
        request: Request<Streaming<EventBatch>>,
    ) -> Result<Response<Self::StreamEventsStream>, Status> {
        let mut stream = request.into_inner();
        let buffer = Arc::clone(&self.buffer);

        let (tx, rx) = mpsc::channel(32);

        tokio::spawn(async move {
            while let Ok(Some(batch)) = stream.message().await {
                let events: Vec<Event> = batch.events;
                let event_ids: Vec<String> = events.iter().map(|e| e.id.clone()).collect();

                let dropped = buffer.push(events);

                if dropped > 0 {
                    info!(dropped = dropped, "Buffer overflow, events dropped");
                }

                let ack = Ack {
                    event_ids,
                    errors: vec![],
                    buffer_size: buffer.len() as i64,
                    buffer_capacity: buffer.capacity() as i64,
                };

                if tx.send(Ok(ack)).await.is_err() {
                    break;
                }
            }
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn send_event(&self, request: Request<Event>) -> Result<Response<Ack>, Status> {
        let event = request.into_inner();
        let event_id = event.id.clone();

        let dropped = self.buffer.push(vec![event]);

        if dropped > 0 {
            return Err(Status::resource_exhausted("Buffer full, event dropped"));
        }

        Ok(Response::new(Ack {
            event_ids: vec![event_id],
            errors: vec![],
            buffer_size: self.buffer.len() as i64,
            buffer_capacity: self.buffer.capacity() as i64,
        }))
    }

    async fn health(
        &self,
        _request: Request<HealthRequest>,
    ) -> Result<Response<HealthResponse>, Status> {
        use crate::proto::ComponentHealth;

        let buffer_health = ComponentHealth {
            healthy: !self.buffer.is_full(),
            message: format!(
                "Buffer: {}/{}",
                self.buffer.len(),
                self.buffer.capacity()
            ),
        };

        let mut components = HashMap::new();
        components.insert("buffer".to_string(), buffer_health);

        Ok(Response::new(HealthResponse {
            healthy: true,
            components,
            uptime_seconds: self.start_time.elapsed().as_secs() as i64,
            events_processed: self.events_processed.load(Ordering::Relaxed),
        }))
    }
}
