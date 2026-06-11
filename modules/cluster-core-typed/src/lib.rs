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
mod cluster_singleton_config;
mod cluster_state_subscription;
mod cluster_state_subscription_result;
mod self_removed;
mod self_up;

pub use cluster::Cluster;
pub use cluster_command::ClusterCommand;
pub use cluster_event_subscription::ClusterEventSubscription;
pub use cluster_identity::ClusterIdentity;
pub use cluster_setup::ClusterSetup;
pub use cluster_singleton_config::ClusterSingletonConfig;
pub use cluster_state_subscription::ClusterStateSubscription;
pub use cluster_state_subscription_result::ClusterStateSubscriptionResult;
pub use self_removed::SelfRemoved;
pub use self_up::SelfUp;
