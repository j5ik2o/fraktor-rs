//! Recovery configuration for persistent actors.

use crate::core::snapshot_selection_criteria::SnapshotSelectionCriteria;

/// Recovery configuration used by persistent actors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Recovery {
  from_snapshot:  SnapshotSelectionCriteria,
  to_sequence_nr: u64,
  replay_max:     u64,
}

impl Recovery {
  /// Creates a recovery configuration with explicit bounds.
  #[must_use]
  pub const fn new(from_snapshot: SnapshotSelectionCriteria, to_sequence_nr: u64, replay_max: u64) -> Self {
    Self { from_snapshot, to_sequence_nr, replay_max }
  }

  /// Returns the snapshot selection criteria.
  #[must_use]
  pub const fn from_snapshot(&self) -> SnapshotSelectionCriteria {
    self.from_snapshot
  }

  /// Returns the upper sequence number bound.
  #[must_use]
  pub const fn to_sequence_nr(&self) -> u64 {
    self.to_sequence_nr
  }

  /// Returns the maximum number of events to replay.
  #[must_use]
  pub const fn replay_max(&self) -> u64 {
    self.replay_max
  }

  /// Returns recovery configuration that skips snapshot and event replay.
  #[must_use]
  pub const fn none() -> Self {
    Self { from_snapshot: SnapshotSelectionCriteria::none(), to_sequence_nr: 0, replay_max: 0 }
  }

  /// Returns `true` when recovery is disabled.
  #[must_use]
  pub const fn is_disabled(&self) -> bool {
    self.to_sequence_nr == 0 && self.from_snapshot.is_none()
  }
}

impl Default for Recovery {
  fn default() -> Self {
    Self { from_snapshot: SnapshotSelectionCriteria::latest(), to_sequence_nr: u64::MAX, replay_max: u64::MAX }
  }
}
