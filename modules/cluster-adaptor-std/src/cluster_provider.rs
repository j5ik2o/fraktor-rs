//! Cluster provider adaptors for std runtimes.

#[cfg(feature = "aws-ecs")]
mod aws_ecs_cluster_provider;
#[cfg(feature = "aws-ecs")]
mod ecs_cluster_config;
#[cfg(feature = "aws-ecs")]
mod ecs_poller_error;
#[cfg(feature = "aws-ecs")]
mod ecs_task_discovery;
mod local_cluster_provider_ext;

#[cfg(feature = "aws-ecs")]
pub use aws_ecs_cluster_provider::AwsEcsClusterProvider;
#[cfg(feature = "aws-ecs")]
pub use ecs_cluster_config::EcsClusterConfig;
#[cfg(feature = "aws-ecs")]
pub use ecs_poller_error::EcsPollerError;
pub use local_cluster_provider_ext::{subscribe_remoting_events, wrap_local_cluster_provider};
