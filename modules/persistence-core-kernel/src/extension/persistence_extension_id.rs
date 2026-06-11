//! Extension identifier for persistence subsystem.

use fraktor_actor_core_kernel_rs::{actor::extension::ExtensionId, system::ActorSystem};

use crate::{
  config::PersistenceConfig,
  extension::{PersistenceExtension, PersistenceExtensionShared},
  journal::Journal,
  snapshot::SnapshotStore,
};

/// Registers and instantiates persistence extensions.
pub struct PersistenceExtensionId<J, S> {
  journal:        J,
  snapshot_store: S,
  config:         PersistenceConfig,
}

impl<J, S> PersistenceExtensionId<J, S> {
  /// Creates a new identifier with the provided journal and snapshot store.
  #[must_use]
  pub const fn new(journal: J, snapshot_store: S) -> Self {
    Self::new_with_config(journal, snapshot_store, PersistenceConfig::default_config())
  }

  /// Creates a new identifier with explicit persistence configuration.
  #[must_use]
  pub const fn new_with_config(journal: J, snapshot_store: S, config: PersistenceConfig) -> Self {
    Self { journal, snapshot_store, config }
  }
}

impl<J, S> ExtensionId for PersistenceExtensionId<J, S>
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
  type Ext = PersistenceExtensionShared;

  fn create_extension(&self, system: &ActorSystem) -> Self::Ext {
    let extension = match PersistenceExtension::new_with_config(
      system,
      self.journal.clone(),
      self.snapshot_store.clone(),
      self.config,
    ) {
      | Ok(extension) => extension,
      | Err(error) => {
        panic!("persistence extension bootstrap failed: {error:?}");
      },
    };
    PersistenceExtensionShared::new(extension)
  }
}
