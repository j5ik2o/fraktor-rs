//! Proxy layer for forwarding persistence operations to target plugins.

#[cfg(test)]
mod tests;

use fraktor_utils_rs::core::sync::ArcShared;

use crate::core::{
  journal::Journal, persistent_repr::PersistentRepr, snapshot_metadata::SnapshotMetadata,
  snapshot_selection_criteria::SnapshotSelectionCriteria, snapshot_store::SnapshotStore,
};

/// Proxy that forwards journal and snapshot-store operations to the current target plugins.
#[derive(Clone, Debug)]
pub struct PersistencePluginProxy<J, S> {
  journal:        J,
  snapshot_store: S,
}

impl<J, S> PersistencePluginProxy<J, S> {
  /// Creates a new proxy for the provided journal and snapshot-store plugins.
  #[must_use]
  pub const fn new(journal: J, snapshot_store: S) -> Self {
    Self { journal, snapshot_store }
  }

  /// Replaces the target plugins used by this proxy.
  pub fn set_target(&mut self, journal: J, snapshot_store: S) {
    self.journal = journal;
    self.snapshot_store = snapshot_store;
  }
}

impl<J, S> Journal for PersistencePluginProxy<J, S>
where
  J: Journal,
  S: Send + Sync + 'static,
{
  type DeleteFuture<'a>
    = J::DeleteFuture<'a>
  where
    Self: 'a;
  type HighestSeqNrFuture<'a>
    = J::HighestSeqNrFuture<'a>
  where
    Self: 'a;
  type ReplayFuture<'a>
    = J::ReplayFuture<'a>
  where
    Self: 'a;
  type WriteFuture<'a>
    = J::WriteFuture<'a>
  where
    Self: 'a;

  fn write_messages<'a>(&'a mut self, messages: &'a [PersistentRepr]) -> Self::WriteFuture<'a> {
    self.journal.write_messages(messages)
  }

  fn replay_messages<'a>(
    &'a self,
    persistence_id: &'a str,
    from_sequence_nr: u64,
    to_sequence_nr: u64,
    max: u64,
  ) -> Self::ReplayFuture<'a> {
    self.journal.replay_messages(persistence_id, from_sequence_nr, to_sequence_nr, max)
  }

  fn delete_messages_to<'a>(&'a mut self, persistence_id: &'a str, to_sequence_nr: u64) -> Self::DeleteFuture<'a> {
    self.journal.delete_messages_to(persistence_id, to_sequence_nr)
  }

  fn highest_sequence_nr<'a>(&'a self, persistence_id: &'a str) -> Self::HighestSeqNrFuture<'a> {
    self.journal.highest_sequence_nr(persistence_id)
  }
}

impl<J, S> SnapshotStore for PersistencePluginProxy<J, S>
where
  J: Send + Sync + 'static,
  S: SnapshotStore,
{
  type DeleteManyFuture<'a>
    = S::DeleteManyFuture<'a>
  where
    Self: 'a;
  type DeleteOneFuture<'a>
    = S::DeleteOneFuture<'a>
  where
    Self: 'a;
  type LoadFuture<'a>
    = S::LoadFuture<'a>
  where
    Self: 'a;
  type SaveFuture<'a>
    = S::SaveFuture<'a>
  where
    Self: 'a;

  fn save_snapshot<'a>(
    &'a mut self,
    metadata: SnapshotMetadata,
    snapshot: ArcShared<dyn core::any::Any + Send + Sync>,
  ) -> Self::SaveFuture<'a> {
    self.snapshot_store.save_snapshot(metadata, snapshot)
  }

  fn load_snapshot<'a>(&'a self, persistence_id: &'a str, criteria: SnapshotSelectionCriteria) -> Self::LoadFuture<'a> {
    self.snapshot_store.load_snapshot(persistence_id, criteria)
  }

  fn delete_snapshot<'a>(&'a mut self, metadata: &'a SnapshotMetadata) -> Self::DeleteOneFuture<'a> {
    self.snapshot_store.delete_snapshot(metadata)
  }

  fn delete_snapshots<'a>(
    &'a mut self,
    persistence_id: &'a str,
    criteria: SnapshotSelectionCriteria,
  ) -> Self::DeleteManyFuture<'a> {
    self.snapshot_store.delete_snapshots(persistence_id, criteria)
  }
}
