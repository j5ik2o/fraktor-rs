//! Installer for the persistence extension.

#[cfg(test)]
#[path = "persistence_extension_installer_test.rs"]
mod tests;

use alloc::format;

use fraktor_actor_core_kernel_rs::{
  actor::extension::{ExtensionInstaller, install_extension_id},
  serialization::contribution::register_serialization_registry_contributor,
  system::{ActorSystem, ActorSystemBuildError},
};

use crate::{
  config::PersistenceConfig, extension::PersistenceExtensionId, journal::Journal,
  serialization::PersistenceSerializationContributor, snapshot::SnapshotStore,
};

/// Installs the persistence extension into the actor system.
pub struct PersistenceExtensionInstaller<J, S> {
  journal:        J,
  snapshot_store: S,
  settings:       PersistenceConfig,
}

impl<J, S> PersistenceExtensionInstaller<J, S> {
  /// Creates a new installer with the provided journal and snapshot store.
  #[must_use]
  pub const fn new(journal: J, snapshot_store: S) -> Self {
    Self::new_with_settings(journal, snapshot_store, PersistenceConfig::default_config())
  }

  /// Creates a new installer with explicit persistence settings.
  #[must_use]
  pub const fn new_with_settings(journal: J, snapshot_store: S, settings: PersistenceConfig) -> Self {
    Self { journal, snapshot_store, settings }
  }
}

impl<J, S> ExtensionInstaller for PersistenceExtensionInstaller<J, S>
where
  J: Journal + Clone + Send + Sync + 'static,
  S: SnapshotStore + Clone + Send + Sync + 'static,
  for<'a> J::WriteFuture<'a>: Send + 'static,
  for<'a> J::ReplayFuture<'a>: Send + 'static,
  for<'a> J::DeleteFuture<'a>: Send + 'static,
  for<'a> J::HighestSeqNrFuture<'a>: Send + 'static,
  for<'a> S::SaveFuture<'a>: Send + 'static,
  for<'a> S::LoadFuture<'a>: Send + 'static,
  for<'a> S::DeleteOneFuture<'a>: Send + 'static,
  for<'a> S::DeleteManyFuture<'a>: Send + 'static,
{
  fn install(&self, system: &ActorSystem) -> Result<(), ActorSystemBuildError> {
    register_serialization_registry_contributor(system, PersistenceSerializationContributor::new()).map_err(
      |error| ActorSystemBuildError::Configuration(format!("persistence serialization registration failed: {error}")),
    )?;
    let extension_id =
      PersistenceExtensionId::new_with_settings(self.journal.clone(), self.snapshot_store.clone(), self.settings);
    install_extension_id(system, &extension_id);
    Ok(())
  }
}
