//! Kubernetes deployment fixtures for POLKU
//!
//! Use Seppo to deploy POLKU gateway to Kubernetes.
//!
//! # Example
//!
//! ```ignore
//! use polku_gateway::deploy::PolkuDeployment;
//! use seppo::Context;
//!
//! let ctx = Context::new().await?;
//!
//! let polku = PolkuDeployment::new("my-polku")
//!     .replicas(3)
//!     .grpc_port(50051)
//!     .build();
//!
//! ctx.apply(&polku.deployment).await?;
//! ctx.apply(&polku.service).await?;
//! ctx.wait_ready("deployment/my-polku").await?;
//! ```

use k8s_openapi::api::apps::v1::{Deployment, DeploymentSpec};
use k8s_openapi::api::core::v1::{
    Container, ContainerPort, EnvVar, PodSpec, PodTemplateSpec, Service, ServicePort, ServiceSpec,
};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::LabelSelector;
use std::collections::BTreeMap;

/// Builder for POLKU Kubernetes deployment
#[derive(Debug, Clone)]
pub struct PolkuDeployment {
    name: String,
    namespace: String,
    image: String,
    replicas: i32,
    grpc_port: i32,
    env_vars: Vec<EnvVar>,
    labels: BTreeMap<String, String>,
}

impl PolkuDeployment {
    /// Create a new POLKU deployment builder
    pub fn new(name: impl Into<String>) -> Self {
        let name = name.into();
        let mut labels = BTreeMap::new();
        labels.insert("app".into(), name.clone());
        labels.insert("app.kubernetes.io/name".into(), "polku".into());
        labels.insert("app.kubernetes.io/component".into(), "gateway".into());

        Self {
            name,
            namespace: "default".into(),
            image: "polku-gateway:latest".into(),
            replicas: 1,
            grpc_port: 50051,
            env_vars: vec![],
            labels,
        }
    }

    /// Set namespace
    pub fn namespace(mut self, ns: impl Into<String>) -> Self {
        self.namespace = ns.into();
        self
    }

    /// Set container image
    pub fn image(mut self, image: impl Into<String>) -> Self {
        self.image = image.into();
        self
    }

    /// Set replica count
    pub fn replicas(mut self, n: i32) -> Self {
        self.replicas = n;
        self
    }

    /// Set gRPC port
    pub fn grpc_port(mut self, port: i32) -> Self {
        self.grpc_port = port;
        self
    }

    /// Add environment variable
    pub fn env(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.env_vars.push(EnvVar {
            name: name.into(),
            value: Some(value.into()),
            value_from: None,
        });
        self
    }

    /// Add custom label
    pub fn label(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.labels.insert(key.into(), value.into());
        self
    }

    /// Build the Kubernetes Deployment resource
    pub fn build_deployment(&self) -> Deployment {
        Deployment {
            metadata: kube::api::ObjectMeta {
                name: Some(self.name.clone()),
                namespace: Some(self.namespace.clone()),
                labels: Some(self.labels.clone()),
                ..Default::default()
            },
            spec: Some(DeploymentSpec {
                replicas: Some(self.replicas),
                selector: LabelSelector {
                    match_labels: Some(
                        [("app".to_string(), self.name.clone())]
                            .into_iter()
                            .collect(),
                    ),
                    ..Default::default()
                },
                template: PodTemplateSpec {
                    metadata: Some(kube::api::ObjectMeta {
                        labels: Some(self.labels.clone()),
                        ..Default::default()
                    }),
                    spec: Some(PodSpec {
                        containers: vec![Container {
                            name: "polku".into(),
                            image: Some(self.image.clone()),
                            ports: Some(vec![ContainerPort {
                                container_port: self.grpc_port,
                                name: Some("grpc".into()),
                                protocol: Some("TCP".into()),
                                ..Default::default()
                            }]),
                            env: if self.env_vars.is_empty() {
                                None
                            } else {
                                Some(self.env_vars.clone())
                            },
                            ..Default::default()
                        }],
                        ..Default::default()
                    }),
                },
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    /// Build the Kubernetes Service resource
    pub fn build_service(&self) -> Service {
        Service {
            metadata: kube::api::ObjectMeta {
                name: Some(self.name.clone()),
                namespace: Some(self.namespace.clone()),
                labels: Some(self.labels.clone()),
                ..Default::default()
            },
            spec: Some(ServiceSpec {
                selector: Some(
                    [("app".to_string(), self.name.clone())]
                        .into_iter()
                        .collect(),
                ),
                ports: Some(vec![ServicePort {
                    port: self.grpc_port,
                    target_port: Some(
                        k8s_openapi::apimachinery::pkg::util::intstr::IntOrString::Int(
                            self.grpc_port,
                        ),
                    ),
                    name: Some("grpc".into()),
                    protocol: Some("TCP".into()),
                    ..Default::default()
                }]),
                type_: Some("ClusterIP".into()),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    /// Build both Deployment and Service
    pub fn build(self) -> PolkuResources {
        PolkuResources {
            deployment: self.build_deployment(),
            service: self.build_service(),
        }
    }
}

/// Container for POLKU K8s resources
pub struct PolkuResources {
    pub deployment: Deployment,
    pub service: Service,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_polku_deployment_builder() {
        let polku = PolkuDeployment::new("my-polku")
            .namespace("prod")
            .image("polku:v1.0")
            .replicas(3)
            .grpc_port(8080)
            .env("LOG_LEVEL", "info")
            .label("env", "production");

        let resources = polku.build();

        // Check deployment
        assert_eq!(
            resources.deployment.metadata.name,
            Some("my-polku".to_string())
        );
        assert_eq!(
            resources.deployment.metadata.namespace,
            Some("prod".to_string())
        );
        assert_eq!(
            resources.deployment.spec.as_ref().unwrap().replicas,
            Some(3)
        );

        // Check service
        assert_eq!(
            resources.service.metadata.name,
            Some("my-polku".to_string())
        );
    }

    #[test]
    fn test_polku_deployment_defaults() {
        let polku = PolkuDeployment::new("test").build();

        assert_eq!(polku.deployment.spec.as_ref().unwrap().replicas, Some(1));

        let container = &polku
            .deployment
            .spec
            .as_ref()
            .unwrap()
            .template
            .spec
            .as_ref()
            .unwrap()
            .containers[0];
        assert_eq!(container.image, Some("polku-gateway:latest".to_string()));
        assert_eq!(container.ports.as_ref().unwrap()[0].container_port, 50051);
    }
}
