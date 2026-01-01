//! Installer for the persistence extension.

#[cfg(test)]
mod tests;

use alloc::format;

use fraktor_actor_rs::core::{
  extension::ExtensionInstaller,
  system::{ActorSystemBuildError, ActorSystemGeneric},
};
use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

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

impl<TB, J, S> ExtensionInstaller<TB> for PersistenceExtensionInstaller<J, S>
where
  TB: RuntimeToolbox + 'static,
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
  fn install(&self, system: &ActorSystemGeneric<TB>) -> Result<(), ActorSystemBuildError> {
    let extension_id = PersistenceExtensionId::new(self.journal.clone(), self.snapshot_store.clone());
    system.extended().register_extension(&extension_id).map(|_| ()).map_err(|error| {
      ActorSystemBuildError::Configuration(format!("persistence extension registration failed: {error:?}"))
    })
  }
}
