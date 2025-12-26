//! std-only adapters for the cluster runtime.

#[cfg(feature = "aws-ecs")]
mod aws_ecs_cluster_provider;
mod cluster_api;
mod gossip_wire_delta_v1;
mod gossip_wire_node_record;
mod grain_ref;
mod grain_std_call_options;
mod local_cluster_provider_ext;
mod membership_coordinator_driver;
mod pub_sub_delivery_actor;
mod tokio_gossip_transport;
mod tokio_gossip_transport_config;
mod tokio_gossiper;
mod tokio_gossiper_config;

#[cfg(feature = "aws-ecs")]
pub use aws_ecs_cluster_provider::{AwsEcsClusterProvider, EcsClusterConfig, EcsPollerError};
pub use cluster_api::ClusterApi;
pub use grain_ref::GrainRef;
pub use grain_std_call_options::{call_options_with_retry, call_options_with_timeout, default_grain_call_options};
pub use local_cluster_provider_ext::{subscribe_remoting_events, wrap_local_cluster_provider};
pub use membership_coordinator_driver::MembershipCoordinatorDriverGeneric;
pub use pub_sub_delivery_actor::PubSubDeliveryActor;
pub use tokio_gossip_transport::TokioGossipTransport;
pub use tokio_gossip_transport_config::TokioGossipTransportConfig;
pub use tokio_gossiper::TokioGossiper;
pub use tokio_gossiper_config::TokioGossiperConfig;
