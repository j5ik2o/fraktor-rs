//! Cluster provider adaptors for std runtimes.

#[cfg(feature = "aws-ecs")]
mod aws_ecs_cluster_provider;
/// Generic discovery backend execution contract.
mod discovery_backend;
/// Observable generic discovery backend failure.
mod discovery_backend_error;
#[cfg(feature = "aws-ecs")]
mod ecs_cluster_config;
#[cfg(feature = "aws-ecs")]
mod ecs_poller_error;
#[cfg(feature = "aws-ecs")]
mod ecs_task_discovery;
/// Generic discovery backend adapter contract.
mod generic_discovery_adapter;
mod local_cluster_provider_ext;

#[cfg(feature = "aws-ecs")]
pub use aws_ecs_cluster_provider::AwsEcsClusterProvider;
pub use discovery_backend::DiscoveryBackend;
pub use discovery_backend_error::DiscoveryBackendError;
#[cfg(feature = "aws-ecs")]
pub use ecs_cluster_config::EcsClusterConfig;
#[cfg(feature = "aws-ecs")]
pub use ecs_poller_error::EcsPollerError;
pub use generic_discovery_adapter::GenericDiscoveryAdapter;
pub use local_cluster_provider_ext::{subscribe_remoting_events, wrap_local_cluster_provider};
