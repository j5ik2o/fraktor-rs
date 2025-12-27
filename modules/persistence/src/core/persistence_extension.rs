//! Persistence extension wiring for actor systems.

use alloc::boxed::Box;
use core::marker::PhantomData;

use fraktor_actor_rs::core::extension::Extension;
use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::{ArcShared, sync_mutex_like::SyncMutexLike},
};

use crate::core::{
  journal::Journal,
  journal_error::JournalError,
  persistence_settings::PersistenceSettings,
  persistent_repr::PersistentRepr,
  snapshot_metadata::SnapshotMetadata,
  snapshot_selection_criteria::SnapshotSelectionCriteria,
  snapshot_store::{SnapshotLoadResult, SnapshotStore},
  snapshot_store_error::SnapshotStoreError,
};

/// Persistence extension type alias for the default toolbox.
pub type PersistenceExtension = PersistenceExtensionGeneric<NoStdToolbox>;

/// Persistence extension registered within the actor system.
pub struct PersistenceExtensionGeneric<TB: RuntimeToolbox + 'static> {
  journal:        JournalSharedGeneric<TB>,
  snapshot_store: SnapshotStoreSharedGeneric<TB>,
  settings:       PersistenceSettings,
  _marker:        PhantomData<TB>,
}

impl<TB: RuntimeToolbox + 'static> PersistenceExtensionGeneric<TB> {
  /// Creates a new persistence extension from concrete plugins.
  #[must_use]
  pub fn new(journal: Box<dyn Journal>, snapshot_store: Box<dyn SnapshotStore>, settings: PersistenceSettings) -> Self {
    Self {
      journal: JournalSharedGeneric::new(journal),
      snapshot_store: SnapshotStoreSharedGeneric::new(snapshot_store),
      settings,
      _marker: PhantomData,
    }
  }

  /// Returns the shared settings.
  #[must_use]
  pub const fn settings(&self) -> &PersistenceSettings {
    &self.settings
  }

  /// Writes messages to the journal.
  ///
  /// # Errors
  ///
  /// Returns an error when the journal write fails.
  pub fn write_messages(&self, messages: &[PersistentRepr]) -> Result<(), JournalError> {
    self.journal.with_write(|journal| journal.write_messages(messages))
  }

  /// Replays messages from the journal.
  ///
  /// # Errors
  ///
  /// Returns an error when replay fails.
  pub fn replay_messages(
    &self,
    persistence_id: &str,
    from_sequence_nr: u64,
    to_sequence_nr: u64,
    max: u64,
  ) -> Result<(alloc::vec::Vec<PersistentRepr>, u64), JournalError> {
    self.journal.with_read(|journal| journal.replay_messages(persistence_id, from_sequence_nr, to_sequence_nr, max))
  }

  /// Deletes messages from the journal.
  ///
  /// # Errors
  ///
  /// Returns an error when deletion fails.
  pub fn delete_messages_to(&self, persistence_id: &str, to_sequence_nr: u64) -> Result<(), JournalError> {
    self.journal.with_write(|journal| journal.delete_messages_to(persistence_id, to_sequence_nr))
  }

  /// Returns the highest stored sequence number.
  ///
  /// # Errors
  ///
  /// Returns an error when the query fails.
  pub fn highest_sequence_nr(&self, persistence_id: &str) -> Result<u64, JournalError> {
    self.journal.with_read(|journal| journal.highest_sequence_nr(persistence_id))
  }

  /// Loads a snapshot from the snapshot store.
  ///
  /// # Errors
  ///
  /// Returns an error when loading fails.
  pub fn load_snapshot(
    &self,
    persistence_id: &str,
    criteria: SnapshotSelectionCriteria,
    to_sequence_nr: u64,
  ) -> Result<SnapshotLoadResult, SnapshotStoreError> {
    self.snapshot_store.with_read(|store| store.load_snapshot(persistence_id, criteria, to_sequence_nr))
  }

  /// Saves a snapshot to the snapshot store.
  ///
  /// # Errors
  ///
  /// Returns an error when saving fails.
  pub fn save_snapshot(
    &self,
    metadata: SnapshotMetadata,
    snapshot: ArcShared<dyn core::any::Any + Send + Sync>,
  ) -> Result<(), SnapshotStoreError> {
    self.snapshot_store.with_write(|store| store.save_snapshot(metadata, snapshot))
  }

  /// Deletes a snapshot from the snapshot store.
  ///
  /// # Errors
  ///
  /// Returns an error when deletion fails.
  pub fn delete_snapshot(&self, metadata: &SnapshotMetadata) -> Result<(), SnapshotStoreError> {
    self.snapshot_store.with_write(|store| store.delete_snapshot(metadata))
  }

  /// Deletes snapshots matching the criteria.
  ///
  /// # Errors
  ///
  /// Returns an error when deletion fails.
  pub fn delete_snapshots(
    &self,
    persistence_id: &str,
    criteria: SnapshotSelectionCriteria,
  ) -> Result<(), SnapshotStoreError> {
    self.snapshot_store.with_write(|store| store.delete_snapshots(persistence_id, criteria))
  }
}

impl<TB: RuntimeToolbox + 'static> Extension<TB> for PersistenceExtensionGeneric<TB> {}

struct JournalSharedGeneric<TB: RuntimeToolbox + 'static> {
  inner: ArcShared<ToolboxMutex<Box<dyn Journal>, TB>>,
}

impl<TB: RuntimeToolbox + 'static> JournalSharedGeneric<TB> {
  fn new(journal: Box<dyn Journal>) -> Self {
    let mutex = <TB::MutexFamily as SyncMutexFamily>::create(journal);
    Self { inner: ArcShared::new(mutex) }
  }

  fn with_read<R>(&self, f: impl FnOnce(&dyn Journal) -> R) -> R {
    let guard = self.inner.lock();
    f(&**guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut dyn Journal) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut **guard)
  }
}

struct SnapshotStoreSharedGeneric<TB: RuntimeToolbox + 'static> {
  inner: ArcShared<ToolboxMutex<Box<dyn SnapshotStore>, TB>>,
}

impl<TB: RuntimeToolbox + 'static> SnapshotStoreSharedGeneric<TB> {
  fn new(store: Box<dyn SnapshotStore>) -> Self {
    let mutex = <TB::MutexFamily as SyncMutexFamily>::create(store);
    Self { inner: ArcShared::new(mutex) }
  }

  fn with_read<R>(&self, f: impl FnOnce(&dyn SnapshotStore) -> R) -> R {
    let guard = self.inner.lock();
    f(&**guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut dyn SnapshotStore) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut **guard)
  }
}
