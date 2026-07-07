//! Setup wrapper for replacing the default typed cluster sharding extension.

#[cfg(test)]
#[path = "cluster_sharding_setup_test.rs"]
mod tests;

use core::any::TypeId;

use fraktor_actor_core_kernel_rs::{
  actor::extension::{ExtensionId, ExtensionInstaller},
  system::{ActorSystem, ActorSystemBuildError},
};
use fraktor_actor_core_typed_rs::ExtensionSetup;

use crate::{ClusterSharding, ClusterShardingId};

/// Replaces the default [`ClusterSharding`] extension during actor-system startup.
///
/// Intended for tests that need to substitute the extension with a stub or mock
/// implementation, mirroring Pekko's `ClusterShardingSetup`.
#[derive(Clone)]
pub struct ClusterShardingSetup {
  inner: ExtensionSetup<ClusterShardingId>,
}

impl ClusterShardingSetup {
  /// Creates a new setup with a custom [`ClusterSharding`] factory.
  #[must_use]
  pub fn new<F>(create_extension: F) -> Self
  where
    F: Fn(&ActorSystem) -> ClusterSharding + Send + Sync + 'static, {
    Self { inner: ExtensionSetup::new(ClusterShardingId::new(), create_extension) }
  }
}

impl ExtensionId for ClusterShardingSetup {
  type Ext = ClusterSharding;

  fn create_extension(&self, system: &ActorSystem) -> Self::Ext {
    self.inner.create_extension(system)
  }

  fn id(&self) -> TypeId {
    self.inner.id()
  }
}

impl ExtensionInstaller for ClusterShardingSetup {
  fn install(&self, system: &ActorSystem) -> Result<(), ActorSystemBuildError> {
    self.inner.install(system)
  }
}
