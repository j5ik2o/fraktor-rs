#![deny(missing_docs)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]
#![deny(rustdoc::broken_intra_doc_links)]
#![deny(unsafe_op_in_unsafe_fn)]
#![deny(unreachable_pub)]
#![allow(unknown_lints)]
#![deny(cfg_std_forbid)]
#![cfg_attr(not(test), no_std)]

//! Typed wrappers over the cluster kernel.

extern crate alloc;

mod cluster;
mod cluster_command;
mod cluster_event_subscription;
mod cluster_identity;
mod cluster_setup;
mod cluster_sharding;
mod cluster_sharding_setup;
mod cluster_singleton_config;
mod cluster_state_subscription;
mod cluster_state_subscription_result;
mod distributed_data;
mod entity;
mod entity_context;
mod grain_ref;
mod grain_type_key;
mod replicator_command;
mod replicator_message_adapter;
mod self_removed;
mod self_up;
mod update_modify_fn;

pub use cluster::Cluster;
pub use cluster_command::ClusterCommand;
pub use cluster_event_subscription::ClusterEventSubscription;
pub use cluster_identity::ClusterIdentity;
pub use cluster_setup::ClusterSetup;
pub use cluster_sharding::{ClusterSharding, ClusterShardingId, EntityRegion};
pub use cluster_sharding_setup::ClusterShardingSetup;
pub use cluster_singleton_config::ClusterSingletonConfig;
pub use cluster_state_subscription::ClusterStateSubscription;
pub use cluster_state_subscription_result::ClusterStateSubscriptionResult;
pub use distributed_data::{DEFAULT_UNEXPECTED_ASK_TIMEOUT, DistributedData, DistributedDataId};
pub use entity::{CreateBehaviorFn, Entity};
pub use entity_context::EntityContext;
pub use grain_ref::GrainRef;
pub use grain_type_key::GrainTypeKey;
pub use replicator_command::ReplicatorCommand;
pub use replicator_message_adapter::ReplicatorMessageAdapter;
pub use self_removed::SelfRemoved;
pub use self_up::SelfUp;
pub use update_modify_fn::UpdateModifyFn;
