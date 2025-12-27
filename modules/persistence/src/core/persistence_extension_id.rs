//! Extension identifier for the persistence subsystem.

use alloc::boxed::Box;

use fraktor_actor_rs::core::{extension::ExtensionId, system::ActorSystemGeneric};
use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use crate::core::{
  in_memory_journal::InMemoryJournal, in_memory_snapshot_store::InMemorySnapshotStore,
  persistence_extension::PersistenceExtensionGeneric, persistence_settings::PersistenceSettings,
};

/// Identifier used to register the persistence extension.
#[derive(Clone, Debug, Default)]
pub struct PersistenceExtensionId {
  settings: PersistenceSettings,
}

impl PersistenceExtensionId {
  /// Creates a new identifier with the provided settings.
  #[must_use]
  pub const fn new(settings: PersistenceSettings) -> Self {
    Self { settings }
  }
}

impl<TB: RuntimeToolbox + 'static> ExtensionId<TB> for PersistenceExtensionId {
  type Ext = PersistenceExtensionGeneric<TB>;

  fn create_extension(&self, _system: &ActorSystemGeneric<TB>) -> Self::Ext {
    let journal: Box<dyn crate::core::journal::Journal> = Box::new(InMemoryJournal::default());
    let snapshot_store: Box<dyn crate::core::snapshot_store::SnapshotStore> =
      Box::new(InMemorySnapshotStore::default());
    PersistenceExtensionGeneric::new(journal, snapshot_store, self.settings.clone())
  }
}
