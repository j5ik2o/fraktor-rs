//! Snapshot selection criteria.

#[cfg(test)]
mod tests;

use core::cmp;

use crate::core::snapshot_metadata::SnapshotMetadata;

/// Criteria used when selecting snapshots.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotSelectionCriteria {
  max_sequence_nr: u64,
  max_timestamp:   u64,
  min_sequence_nr: u64,
  min_timestamp:   u64,
}

impl SnapshotSelectionCriteria {
  /// Creates a new selection criteria.
  #[must_use]
  pub const fn new(max_sequence_nr: u64, max_timestamp: u64, min_sequence_nr: u64, min_timestamp: u64) -> Self {
    Self { max_sequence_nr, max_timestamp, min_sequence_nr, min_timestamp }
  }

  /// Returns criteria that only matches the latest snapshot.
  #[must_use]
  pub const fn latest() -> Self {
    Self { max_sequence_nr: u64::MAX, max_timestamp: u64::MAX, min_sequence_nr: 0, min_timestamp: 0 }
  }

  /// Returns criteria that matches no snapshots.
  #[must_use]
  pub const fn none() -> Self {
    Self { max_sequence_nr: 0, max_timestamp: 0, min_sequence_nr: 1, min_timestamp: 1 }
  }

  /// Returns true when the metadata matches this criteria.
  #[must_use]
  pub const fn matches(&self, metadata: &SnapshotMetadata) -> bool {
    metadata.sequence_nr() >= self.min_sequence_nr
      && metadata.sequence_nr() <= self.max_sequence_nr
      && metadata.timestamp() >= self.min_timestamp
      && metadata.timestamp() <= self.max_timestamp
  }

  /// Returns a new criteria with a tighter maximum sequence number.
  #[must_use]
  pub fn limit(&self, max_sequence_nr: u64) -> Self {
    Self {
      max_sequence_nr: cmp::min(self.max_sequence_nr, max_sequence_nr),
      max_timestamp:   self.max_timestamp,
      min_sequence_nr: self.min_sequence_nr,
      min_timestamp:   self.min_timestamp,
    }
  }

  /// Returns the maximum sequence number.
  #[must_use]
  pub const fn max_sequence_nr(&self) -> u64 {
    self.max_sequence_nr
  }
}
