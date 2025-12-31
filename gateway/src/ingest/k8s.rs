//! K8s Ingestor - watch Kubernetes resources and emit as Messages
//!
//! Watches Kubernetes events/resources and converts them to POLKU Messages.

use crate::error::PluginError;
use crate::message::Message;
use bytes::Bytes;
use futures::StreamExt;
use k8s_openapi::api::core::v1::Event as K8sEvent;
use kube::Client;
use kube::api::Api;
use kube::runtime::watcher::{self, Event as WatcherEvent};
use tokio::sync::mpsc;

/// Configuration for K8s ingestor
#[derive(Debug, Clone, Default)]
pub struct K8sIngestorConfig {
    /// Namespace to watch (None = all namespaces)
    pub namespace: Option<String>,
    /// Label selector for filtering
    pub label_selector: Option<String>,
    /// Field selector for filtering
    pub field_selector: Option<String>,
}

impl K8sIngestorConfig {
    /// Watch a specific namespace
    pub fn namespace(mut self, ns: impl Into<String>) -> Self {
        self.namespace = Some(ns.into());
        self
    }

    /// Filter by labels
    pub fn labels(mut self, selector: impl Into<String>) -> Self {
        self.label_selector = Some(selector.into());
        self
    }
}

/// Watches Kubernetes events and sends them as Messages
pub struct K8sIngestor {
    client: Client,
    config: K8sIngestorConfig,
}

impl K8sIngestor {
    /// Create from Seppo context
    pub fn from_seppo(ctx: &seppo::Context) -> Self {
        Self {
            client: ctx.client.clone(),
            config: K8sIngestorConfig::default().namespace(&ctx.namespace),
        }
    }

    /// Create with explicit client and config
    pub fn with_config(client: Client, config: K8sIngestorConfig) -> Self {
        Self { client, config }
    }

    /// Create with default config
    pub async fn new() -> Result<Self, PluginError> {
        let client = Client::try_default()
            .await
            .map_err(|e| PluginError::Connection(format!("K8s client: {e}")))?;
        Ok(Self {
            client,
            config: K8sIngestorConfig::default(),
        })
    }

    /// Start watching and send messages to the provided sender
    ///
    /// This runs until the sender is closed or an error occurs.
    pub async fn watch(self, sender: mpsc::Sender<Message>) -> Result<(), PluginError> {
        let api: Api<K8sEvent> = match &self.config.namespace {
            Some(ns) => Api::namespaced(self.client.clone(), ns),
            None => Api::all(self.client.clone()),
        };

        let mut watcher_config = watcher::Config::default();
        if let Some(ref labels) = self.config.label_selector {
            watcher_config = watcher_config.labels(labels);
        }
        if let Some(ref fields) = self.config.field_selector {
            watcher_config = watcher_config.fields(fields);
        }

        let stream = watcher::watcher(api, watcher_config);

        tokio::pin!(stream);

        tracing::info!(
            namespace = ?self.config.namespace,
            "K8s ingestor started watching events"
        );

        while let Some(event) = stream.next().await {
            match event {
                Ok(WatcherEvent::Apply(k8s_event)) | Ok(WatcherEvent::InitApply(k8s_event)) => {
                    if let Some(msg) = self.k8s_event_to_message(&k8s_event)
                        && sender.send(msg).await.is_err()
                    {
                        tracing::debug!("K8s ingestor channel closed");
                        break;
                    }
                }
                Ok(WatcherEvent::Delete(k8s_event)) => {
                    // Optionally emit delete events
                    if let Some(mut msg) = self.k8s_event_to_message(&k8s_event) {
                        msg.metadata.insert("k8s.deleted".into(), "true".into());
                        if sender.send(msg).await.is_err() {
                            break;
                        }
                    }
                }
                Ok(WatcherEvent::Init) | Ok(WatcherEvent::InitDone) => {
                    // Initialization phases
                }
                Err(e) => {
                    tracing::warn!(error = %e, "K8s watcher error");
                    // Continue watching despite errors
                }
            }
        }

        Ok(())
    }

    /// Convert K8s Event to POLKU Message
    fn k8s_event_to_message(&self, event: &K8sEvent) -> Option<Message> {
        let name = event.metadata.name.as_deref()?;
        let namespace = event.metadata.namespace.as_deref().unwrap_or("default");

        let event_type = format!(
            "k8s.{}.{}",
            event.type_.as_deref().unwrap_or("Normal").to_lowercase(),
            event.reason.as_deref().unwrap_or("Unknown")
        );

        let source = format!("k8s/{}/{}", namespace, name);

        let mut msg = Message::new(source, event_type, Bytes::new());

        // Add K8s metadata
        msg.metadata
            .insert("k8s.namespace".into(), namespace.into());
        msg.metadata.insert("k8s.name".into(), name.into());

        if let Some(ref involved) = event.involved_object.name {
            msg.metadata
                .insert("k8s.involved_object".into(), involved.clone());
        }
        if let Some(ref kind) = event.involved_object.kind {
            msg.metadata.insert("k8s.kind".into(), kind.clone());
        }
        if let Some(ref message) = event.message {
            msg.metadata.insert("k8s.message".into(), message.clone());
            // Also set as payload
            msg = Message {
                payload: Bytes::from(message.clone()),
                ..msg
            };
        }
        if let Some(ref reason) = event.reason {
            msg.metadata.insert("k8s.reason".into(), reason.clone());
        }

        Some(msg)
    }
}

/// Spawn K8s ingestor as a background task
pub fn spawn_k8s_ingestor(
    ingestor: K8sIngestor,
    sender: mpsc::Sender<Message>,
) -> tokio::task::JoinHandle<Result<(), PluginError>> {
    tokio::spawn(async move { ingestor.watch(sender).await })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_builder() {
        let config = K8sIngestorConfig::default()
            .namespace("my-ns")
            .labels("app=myapp");

        assert_eq!(config.namespace, Some("my-ns".into()));
        assert_eq!(config.label_selector, Some("app=myapp".into()));
    }

    #[test]
    fn test_config_default() {
        let config = K8sIngestorConfig::default();
        assert!(config.namespace.is_none());
        assert!(config.label_selector.is_none());
    }
}
