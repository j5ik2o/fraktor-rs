//! Extension identifier for persistence subsystem.

use fraktor_actor_rs::core::{extension::ExtensionId, system::ActorSystemGeneric};
use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use crate::core::{
  journal::Journal, persistence_extension::PersistenceExtensionGeneric,
  persistence_extension_shared::PersistenceExtensionSharedGeneric, snapshot_store::SnapshotStore,
};

/// Registers and instantiates persistence extensions.
pub struct PersistenceExtensionId<J, S> {
  journal:        J,
  snapshot_store: S,
}

impl<J, S> PersistenceExtensionId<J, S> {
  /// Creates a new identifier with the provided journal and snapshot store.
  #[must_use]
  pub const fn new(journal: J, snapshot_store: S) -> Self {
    Self { journal, snapshot_store }
  }
}

impl<TB, J, S> ExtensionId<TB> for PersistenceExtensionId<J, S>
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
  type Ext = PersistenceExtensionSharedGeneric<TB>;

  fn create_extension(&self, system: &ActorSystemGeneric<TB>) -> Self::Ext {
    let extension = match PersistenceExtensionGeneric::new(system, self.journal.clone(), self.snapshot_store.clone()) {
      | Ok(extension) => extension,
      | Err(error) => {
        panic!("persistence extension bootstrap failed: {error:?}");
      },
    };
    PersistenceExtensionSharedGeneric::new(extension)
  }
}
