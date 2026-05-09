//! Installer for the persistence extension.

#[cfg(test)]
mod tests;

use fraktor_actor_core_kernel_rs::{
  actor::extension::{ExtensionInstaller, install_extension_id},
  system::{ActorSystem, ActorSystemBuildError},
};

use crate::core::{journal::Journal, persistence_extension_id::PersistenceExtensionId, snapshot_store::SnapshotStore};

/// Installs the persistence extension into the actor system.
pub struct PersistenceExtensionInstaller<J, S> {
  journal:        J,
  snapshot_store: S,
}

impl<J, S> PersistenceExtensionInstaller<J, S> {
  /// Creates a new installer with the provided journal and snapshot store.
  #[must_use]
  pub const fn new(journal: J, snapshot_store: S) -> Self {
    Self { journal, snapshot_store }
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
    let extension_id = PersistenceExtensionId::new(self.journal.clone(), self.snapshot_store.clone());
    install_extension_id(system, &extension_id);
    Ok(())
  }
}
