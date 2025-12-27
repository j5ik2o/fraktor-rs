//! Snapshot selection criteria used during recovery and cleanup.

use crate::core::snapshot_metadata::SnapshotMetadata;

/// Criteria for selecting snapshots during recovery and deletion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SnapshotSelectionCriteria {
  max_sequence_nr: u64,
  max_timestamp:   u64,
  min_sequence_nr: u64,
  min_timestamp:   u64,
}

impl SnapshotSelectionCriteria {
  /// Creates a new criteria set from explicit bounds.
  #[must_use]
  pub const fn new(max_sequence_nr: u64, max_timestamp: u64, min_sequence_nr: u64, min_timestamp: u64) -> Self {
    Self { max_sequence_nr, max_timestamp, min_sequence_nr, min_timestamp }
  }

  /// Returns criteria selecting the latest snapshot.
  #[must_use]
  pub const fn latest() -> Self {
    Self::new(u64::MAX, u64::MAX, 0, 0)
  }

  /// Returns criteria selecting no snapshots.
  #[must_use]
  pub const fn none() -> Self {
    Self::new(0, 0, 0, 0)
  }

  /// Returns `true` when this criteria is set to select no snapshots.
  #[must_use]
  pub const fn is_none(&self) -> bool {
    self.max_sequence_nr == 0 && self.max_timestamp == 0 && self.min_sequence_nr == 0 && self.min_timestamp == 0
  }

  /// Returns the maximum sequence number (inclusive).
  #[must_use]
  pub const fn max_sequence_nr(&self) -> u64 {
    self.max_sequence_nr
  }

  /// Returns the maximum timestamp (inclusive).
  #[must_use]
  pub const fn max_timestamp(&self) -> u64 {
    self.max_timestamp
  }

  /// Returns the minimum sequence number (inclusive).
  #[must_use]
  pub const fn min_sequence_nr(&self) -> u64 {
    self.min_sequence_nr
  }

  /// Returns the minimum timestamp (inclusive).
  #[must_use]
  pub const fn min_timestamp(&self) -> u64 {
    self.min_timestamp
  }

  /// Applies an upper bound based on recovery replay limits.
  #[must_use]
  pub const fn limit(&self, to_sequence_nr: u64) -> Self {
    if to_sequence_nr < self.max_sequence_nr { Self { max_sequence_nr: to_sequence_nr, ..*self } } else { *self }
  }

  /// Returns `true` when the metadata matches the criteria bounds.
  #[must_use]
  pub const fn matches(&self, metadata: &SnapshotMetadata) -> bool {
    metadata.sequence_nr() <= self.max_sequence_nr
      && metadata.timestamp() <= self.max_timestamp
      && metadata.sequence_nr() >= self.min_sequence_nr
      && metadata.timestamp() >= self.min_timestamp
  }
}

impl Default for SnapshotSelectionCriteria {
  fn default() -> Self {
    Self::latest()
  }
}
