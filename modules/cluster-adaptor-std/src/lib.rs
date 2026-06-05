#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]
#![deny(unsafe_op_in_unsafe_fn)]
#![allow(unknown_lints)]

//! Standard adaptors for the fraktor cluster runtime.

extern crate alloc;

/// Cluster provider adaptors for std runtimes.
pub mod cluster_provider;
/// ActorSystem integration for AWS ECS cluster extensions.
#[cfg(feature = "aws-ecs")]
pub mod extension;
/// Std helpers for virtual actor grain APIs.
pub mod grain;
/// Tokio-backed membership and gossip adaptors.
pub mod membership;
/// Cluster message wire frame adaptors.
pub mod message_wire;
/// Cluster publish/subscribe delivery adaptors.
pub mod pub_sub;
