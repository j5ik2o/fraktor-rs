//! Cluster Singleton std adaptors.

mod cluster_singleton_manager_actor;
mod cluster_singleton_proxy_actor;
mod singleton_extension_installer;

pub use cluster_singleton_manager_actor::ClusterSingletonManagerActor;
pub use cluster_singleton_proxy_actor::ClusterSingletonProxyActor;
pub use singleton_extension_installer::ClusterSingletonExtensionInstaller;
