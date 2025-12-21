//! std-only adapters for the cluster runtime.

#[cfg(feature = "aws-ecs")]
mod aws_ecs_cluster_provider;
mod local_cluster_provider_ext;
mod membership_coordinator_driver;

#[cfg(feature = "aws-ecs")]
pub use aws_ecs_cluster_provider::{AwsEcsClusterProvider, EcsClusterConfig, EcsPollerError};
pub use local_cluster_provider_ext::{subscribe_remoting_events, wrap_local_cluster_provider};
pub use membership_coordinator_driver::MembershipCoordinatorDriverGeneric;
