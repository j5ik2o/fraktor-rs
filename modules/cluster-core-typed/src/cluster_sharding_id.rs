//! Identifier for the typed [`ClusterSharding`](crate::ClusterSharding) extension.

use fraktor_actor_core_kernel_rs::actor::extension::ExtensionId;

use crate::cluster_sharding::ClusterSharding;

/// Identifier for the typed [`ClusterSharding`] extension.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub struct ClusterShardingId;

impl ClusterShardingId {
  /// Creates a new cluster sharding extension identifier.
  #[must_use]
  pub const fn new() -> Self {
    Self
  }
}

impl ExtensionId for ClusterShardingId {
  type Ext = ClusterSharding;

  fn create_extension(&self, system: &fraktor_actor_core_kernel_rs::system::ActorSystem) -> Self::Ext {
    Self::Ext::try_from_system(system).unwrap_or_else(|error| {
      panic!("cluster extension must be installed before cluster sharding: {error:?}");
    })
  }
}
