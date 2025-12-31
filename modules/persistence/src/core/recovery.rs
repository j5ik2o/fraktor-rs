//! Recovery configuration.

#[cfg(test)]
mod tests;

use crate::core::snapshot_selection_criteria::SnapshotSelectionCriteria;

/// Recovery configuration for persistent actors.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recovery {
  from_snapshot:  SnapshotSelectionCriteria,
  to_sequence_nr: u64,
  replay_max:     u64,
}

impl Recovery {
  /// Creates a recovery configuration with explicit limits.
  #[must_use]
  pub const fn new(to_sequence_nr: u64, replay_max: u64) -> Self {
    Self { from_snapshot: SnapshotSelectionCriteria::latest(), to_sequence_nr, replay_max }
  }

  /// Creates a recovery configuration with explicit snapshot criteria.
  #[must_use]
  pub const fn from_snapshot(criteria: SnapshotSelectionCriteria) -> Self {
    Self { from_snapshot: criteria, to_sequence_nr: u64::MAX, replay_max: u64::MAX }
  }

  /// Returns a recovery configuration that skips replay.
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
}

impl Default for Recovery {
  fn default() -> Self {
    Self { from_snapshot: SnapshotSelectionCriteria::latest(), to_sequence_nr: u64::MAX, replay_max: u64::MAX }
  }
}
