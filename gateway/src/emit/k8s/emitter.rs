//! K8s Emitter - emit events as Kubernetes resources

use crate::emit::Emitter;
use crate::error::PluginError;
use crate::proto::Event;
use async_trait::async_trait;
use k8s_openapi::api::core::v1::ConfigMap;
use kube::Client;
use kube::api::{Api, PostParams};
use std::collections::BTreeMap;

/// What kind of K8s resource to create for events
#[derive(Debug, Clone, Default)]
pub enum ResourceKind {
    /// Store events as ConfigMap data
    #[default]
    ConfigMap,
    // Future: CustomResource, Secret, etc.
}

/// Configuration for K8s emitter
#[derive(Debug, Clone)]
pub struct K8sEmitterConfig {
    /// Namespace to create resources in
    pub namespace: String,
    /// Prefix for resource names
    pub name_prefix: String,
    /// What kind of resource to create
    pub resource_kind: ResourceKind,
    /// Labels to apply to created resources
    pub labels: BTreeMap<String, String>,
}

impl Default for K8sEmitterConfig {
    fn default() -> Self {
        let mut labels = BTreeMap::new();
        labels.insert("app.kubernetes.io/managed-by".into(), "polku".into());

        Self {
            namespace: "default".into(),
            name_prefix: "polku-event".into(),
            resource_kind: ResourceKind::ConfigMap,
            labels,
        }
    }
}

impl K8sEmitterConfig {
    /// Create config for a specific namespace
    pub fn namespace(mut self, ns: impl Into<String>) -> Self {
        self.namespace = ns.into();
        self
    }

    /// Set resource name prefix
    pub fn prefix(mut self, prefix: impl Into<String>) -> Self {
        self.name_prefix = prefix.into();
        self
    }

    /// Add a label
    pub fn label(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.labels.insert(key.into(), value.into());
        self
    }
}

/// Emitter that creates Kubernetes resources from events
pub struct K8sEmitter {
    #[allow(dead_code)] // Reserved for future operations (patch, delete)
    client: Client,
    config: K8sEmitterConfig,
    api: Api<ConfigMap>,
}

impl K8sEmitter {
    /// Create a new K8sEmitter using Seppo context
    pub fn from_seppo(ctx: &seppo::Context) -> Result<Self, PluginError> {
        let config = K8sEmitterConfig::default().namespace(&ctx.namespace);
        Self::with_config(ctx.client.clone(), config)
    }

    /// Create with explicit client and config
    pub fn with_config(client: Client, config: K8sEmitterConfig) -> Result<Self, PluginError> {
        let api = Api::namespaced(client.clone(), &config.namespace);
        Ok(Self {
            client,
            config,
            api,
        })
    }

    /// Create with default config, connecting to current kubeconfig
    pub async fn new() -> Result<Self, PluginError> {
        let client = Client::try_default()
            .await
            .map_err(|e| PluginError::Connection(format!("K8s client: {e}")))?;
        Self::with_config(client, K8sEmitterConfig::default())
    }

    /// Sanitize name to be K8s DNS subdomain compatible.
    ///
    /// K8s resource names must: be lowercase alphanumeric or '-',
    /// start/end with alphanumeric, max 253 chars.
    fn sanitize_name(raw: &str) -> String {
        let sanitized: String = raw
            .chars()
            .map(|c| {
                let lc = c.to_ascii_lowercase();
                if lc.is_ascii_lowercase() || lc.is_ascii_digit() || lc == '-' {
                    lc
                } else {
                    '-'
                }
            })
            .collect();

        // Trim leading/trailing dashes, truncate to 253 chars
        let trimmed = sanitized.trim_matches('-');
        if trimmed.is_empty() {
            "event".to_string()
        } else if trimmed.len() > 253 {
            trimmed[..253].trim_end_matches('-').to_string()
        } else {
            trimmed.to_string()
        }
    }

    /// Create ConfigMap from event
    fn event_to_configmap(&self, event: &Event) -> ConfigMap {
        let name = Self::sanitize_name(&format!("{}-{}", self.config.name_prefix, &event.id));

        let mut data = BTreeMap::new();
        data.insert("event_id".into(), event.id.clone());
        data.insert("source".into(), event.source.clone());
        data.insert("event_type".into(), event.event_type.clone());
        data.insert("timestamp".into(), event.timestamp_unix_ns.to_string());

        // Store metadata as JSON
        if !event.metadata.is_empty()
            && let Ok(json) = serde_json::to_string(&event.metadata)
        {
            data.insert("metadata".into(), json);
        }

        // Store payload as base64 if non-empty
        if !event.payload.is_empty() {
            data.insert(
                "payload".into(),
                base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &event.payload),
            );
        }

        ConfigMap {
            metadata: kube::api::ObjectMeta {
                name: Some(name),
                namespace: Some(self.config.namespace.clone()),
                labels: Some(self.config.labels.clone()),
                ..Default::default()
            },
            data: Some(data),
            ..Default::default()
        }
    }
}

#[async_trait]
impl Emitter for K8sEmitter {
    fn name(&self) -> &'static str {
        "k8s"
    }

    async fn emit(&self, events: &[Event]) -> Result<(), PluginError> {
        for event in events {
            let cm = self.event_to_configmap(event);
            let name = cm.metadata.name.clone().unwrap_or_default();

            self.api
                .create(&PostParams::default(), &cm)
                .await
                .map_err(|e| {
                    PluginError::Send(format!(
                        "K8s create {name} for event {}: {e}",
                        event.id
                    ))
                })?;

            tracing::debug!(
                resource = "ConfigMap",
                name = %name,
                namespace = %self.config.namespace,
                "created K8s resource from event"
            );
        }

        Ok(())
    }

    async fn health(&self) -> bool {
        // Try to list ConfigMaps as health check
        self.api.list(&Default::default()).await.is_ok()
    }

    async fn shutdown(&self) -> Result<(), PluginError> {
        tracing::info!(namespace = %self.config.namespace, "K8s emitter shutdown");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_builder() {
        let config = K8sEmitterConfig::default()
            .namespace("my-ns")
            .prefix("my-prefix")
            .label("env", "test");

        assert_eq!(config.namespace, "my-ns");
        assert_eq!(config.name_prefix, "my-prefix");
        assert_eq!(config.labels.get("env"), Some(&"test".to_string()));
    }

    #[test]
    fn test_config_default_labels() {
        let config = K8sEmitterConfig::default();
        assert_eq!(
            config.labels.get("app.kubernetes.io/managed-by"),
            Some(&"polku".to_string())
        );
    }

    #[test]
    fn test_resource_kind_default() {
        let kind = ResourceKind::default();
        assert!(matches!(kind, ResourceKind::ConfigMap));
    }

    #[test]
    fn test_sanitize_name() {
        // Basic case
        assert_eq!(K8sEmitter::sanitize_name("polku-event-abc123"), "polku-event-abc123");

        // Uppercase → lowercase
        assert_eq!(K8sEmitter::sanitize_name("Polku-Event-ABC"), "polku-event-abc");

        // Invalid chars → dash
        assert_eq!(K8sEmitter::sanitize_name("event_id@123"), "event-id-123");

        // Trim leading/trailing dashes
        assert_eq!(K8sEmitter::sanitize_name("---event---"), "event");

        // Empty → fallback
        assert_eq!(K8sEmitter::sanitize_name("___"), "event");

        // Long name truncated
        let long_name = "a".repeat(300);
        assert!(K8sEmitter::sanitize_name(&long_name).len() <= 253);
    }
}
