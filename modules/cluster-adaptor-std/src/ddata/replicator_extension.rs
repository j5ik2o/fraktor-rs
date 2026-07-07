//! Distributed-data extension installer for std runtimes.

#[cfg(test)]
#[path = "replicator_extension_test.rs"]
mod tests;

use alloc::{format, string::String};

use fraktor_actor_core_kernel_rs::{
  actor::extension::ExtensionInstaller,
  system::{ActorSystem, ActorSystemBuildError},
};
use fraktor_cluster_core_kernel_rs::{ddata::ReplicatorSettings, extension::ClusterExtension};

/// Installs the distributed-data Replicator runtime as an explicit opt-in extension.
pub struct DistributedDataExtensionInstaller {
  settings: ReplicatorSettings,
}

impl DistributedDataExtensionInstaller {
  /// Creates a new installer with the provided Replicator settings.
  #[must_use]
  pub const fn new(settings: ReplicatorSettings) -> Self {
    Self { settings }
  }

  /// Returns the configured Replicator settings.
  #[must_use]
  pub const fn settings(&self) -> &ReplicatorSettings {
    &self.settings
  }
}

impl ExtensionInstaller for DistributedDataExtensionInstaller {
  fn install(&self, system: &ActorSystem) -> Result<(), ActorSystemBuildError> {
    self.settings.validate().map_err(|error| ActorSystemBuildError::Configuration(format!("{error:?}")))?;
    system
      .extended()
      .extension_by_type::<ClusterExtension>()
      .ok_or_else(|| ActorSystemBuildError::Configuration(String::from("cluster extension not installed")))?;
    Ok(())
  }
}
