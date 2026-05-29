//! Typed setup wrapper for installing the cluster extension.

use fraktor_actor_core_kernel_rs::{
  actor::extension::{ExtensionInstaller, install_extension_id},
  system::{ActorSystem, ActorSystemBuildError},
};
use fraktor_cluster_core_kernel_rs::extension::ClusterExtensionId;

/// Typed actor-system setup that installs a cluster extension.
#[derive(Clone)]
pub struct ClusterSetup {
  extension_id: ClusterExtensionId,
}

impl ClusterSetup {
  /// Creates a setup from a cluster extension identifier.
  #[must_use]
  pub const fn new(extension_id: ClusterExtensionId) -> Self {
    Self { extension_id }
  }
}

impl ExtensionInstaller for ClusterSetup {
  fn install(&self, system: &ActorSystem) -> Result<(), ActorSystemBuildError> {
    install_extension_id(system, &self.extension_id);
    Ok(())
  }
}
