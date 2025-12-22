//! std-only adapters for the cluster runtime.

mod activation_executor;
mod activation_storage;
#[cfg(feature = "aws-ecs")]
mod aws_ecs_cluster_provider;
mod local_cluster_provider_ext;
mod membership_coordinator_driver;
mod placement_coordinator_driver;
mod placement_lock;

pub use activation_executor::ActivationExecutor;
pub use activation_storage::ActivationStorage;
#[cfg(feature = "aws-ecs")]
pub use aws_ecs_cluster_provider::{AwsEcsClusterProvider, EcsClusterConfig, EcsPollerError};
pub use local_cluster_provider_ext::{subscribe_remoting_events, wrap_local_cluster_provider};
pub use membership_coordinator_driver::MembershipCoordinatorDriverGeneric;
pub use placement_coordinator_driver::PlacementCoordinatorDriverGeneric;
pub use placement_lock::PlacementLock;
