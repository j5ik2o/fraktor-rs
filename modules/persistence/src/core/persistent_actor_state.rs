//! State container tracking persistent actor progress.

use crate::core::snapshot_metadata::SnapshotMetadata;

/// Mutable persistent actor state owned by the actor implementation.
pub struct PersistentActorState {
  sequence_nr:   u64,
  last_snapshot: Option<SnapshotMetadata>,
}

impl PersistentActorState {
  /// Creates a new state with sequence number set to zero.
  #[must_use]
  pub const fn new() -> Self {
    Self { sequence_nr: 0, last_snapshot: None }
  }

  /// Returns the current sequence number.
  #[must_use]
  pub const fn sequence_nr(&self) -> u64 {
    self.sequence_nr
  }

  /// Updates the sequence number.
  pub const fn set_sequence_nr(&mut self, value: u64) {
    self.sequence_nr = value;
  }

  /// Returns the last saved snapshot metadata, if any.
  #[must_use]
  pub const fn last_snapshot(&self) -> Option<&SnapshotMetadata> {
    self.last_snapshot.as_ref()
  }

  /// Updates the last snapshot metadata.
  pub fn set_last_snapshot(&mut self, metadata: SnapshotMetadata) {
    self.last_snapshot = Some(metadata);
  }
}

impl Default for PersistentActorState {
  fn default() -> Self {
    Self::new()
  }
}
