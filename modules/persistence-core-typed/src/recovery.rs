//! Typed recovery selection configuration.

#[cfg(test)]
#[path = "recovery_test.rs"]
mod tests;

use fraktor_persistence_core_kernel_rs::persistent::Recovery as KernelRecovery;

use crate::SnapshotSelectionCriteria;

/// Recovery selection used by typed persistence actors.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recovery {
  from_snapshot:  SnapshotSelectionCriteria,
  to_sequence_nr: u64,
  replay_max:     u64,
}

impl Recovery {
  /// Creates recovery with explicit replay bounds and latest snapshot selection.
  #[must_use]
  pub const fn new(to_sequence_nr: u64, replay_max: u64) -> Self {
    Self { from_snapshot: SnapshotSelectionCriteria::latest(), to_sequence_nr, replay_max }
  }

  /// Creates recovery from explicit snapshot selection criteria.
  #[must_use]
  pub const fn from_snapshot(criteria: SnapshotSelectionCriteria) -> Self {
    Self { from_snapshot: criteria, to_sequence_nr: u64::MAX, replay_max: u64::MAX }
  }

  /// Creates recovery that replays events without loading snapshots.
  #[must_use]
  pub const fn without_snapshot() -> Self {
    Self { from_snapshot: SnapshotSelectionCriteria::none(), to_sequence_nr: u64::MAX, replay_max: u64::MAX }
  }

  /// Creates recovery that skips both snapshot loading and event replay.
  #[must_use]
  pub const fn none() -> Self {
    Self { from_snapshot: SnapshotSelectionCriteria::none(), to_sequence_nr: 0, replay_max: 0 }
  }

  /// Returns the snapshot selection criteria.
  #[must_use]
  pub const fn snapshot_criteria(&self) -> &SnapshotSelectionCriteria {
    &self.from_snapshot
  }

  /// Returns the maximum sequence number to replay.
  #[must_use]
  pub const fn to_sequence_nr(&self) -> u64 {
    self.to_sequence_nr
  }

  /// Returns the maximum number of events to replay.
  #[must_use]
  pub const fn replay_max(&self) -> u64 {
    self.replay_max
  }

  /// Converts this typed recovery selection to the kernel recovery contract.
  pub(crate) const fn to_kernel(&self) -> KernelRecovery {
    KernelRecovery::from_snapshot(self.from_snapshot.to_kernel())
      .with_replay_bounds(self.to_sequence_nr, self.replay_max)
  }
}

impl Default for Recovery {
  fn default() -> Self {
    Self { from_snapshot: SnapshotSelectionCriteria::latest(), to_sequence_nr: u64::MAX, replay_max: u64::MAX }
  }
}
