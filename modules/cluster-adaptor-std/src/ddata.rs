//! Distributed-data std adaptors.

mod in_memory_durable_store;
mod replicator_actor;
mod replicator_extension;

pub use in_memory_durable_store::InMemoryDurableStore;
pub use replicator_actor::{
  ReplicatorActor, ReplicatorGet, ReplicatorGossipHook, ReplicatorMembershipHook, ReplicatorUpdate,
};
pub use replicator_extension::DistributedDataExtensionInstaller;
