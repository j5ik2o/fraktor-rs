//! Typed recovery snapshot selection criteria.

#[cfg(test)]
#[path = "snapshot_selection_criteria_test.rs"]
mod tests;

use fraktor_persistence_core_kernel_rs::snapshot::SnapshotSelectionCriteria as KernelSnapshotSelectionCriteria;

/// Criteria used to select a snapshot during typed recovery.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotSelectionCriteria {
  max_sequence_nr: u64,
  max_timestamp:   u64,
  min_sequence_nr: u64,
  min_timestamp:   u64,
}

impl SnapshotSelectionCriteria {
  /// Creates snapshot selection criteria with explicit bounds.
  #[must_use]
  pub const fn new(max_sequence_nr: u64, max_timestamp: u64, min_sequence_nr: u64, min_timestamp: u64) -> Self {
    Self { max_sequence_nr, max_timestamp, min_sequence_nr, min_timestamp }
  }

  /// Returns criteria that selects the latest available snapshot.
  #[must_use]
  pub const fn latest() -> Self {
    Self { max_sequence_nr: u64::MAX, max_timestamp: u64::MAX, min_sequence_nr: 0, min_timestamp: 0 }
  }

  /// Returns criteria that selects no snapshots.
  #[must_use]
  pub const fn none() -> Self {
    Self { max_sequence_nr: 0, max_timestamp: 0, min_sequence_nr: 1, min_timestamp: 1 }
  }

  /// Returns criteria bounded by sequence number.
  #[must_use]
  pub const fn to_sequence_nr(max_sequence_nr: u64) -> Self {
    Self { max_sequence_nr, max_timestamp: u64::MAX, min_sequence_nr: 0, min_timestamp: 0 }
  }

  /// Returns criteria bounded by timestamp.
  #[must_use]
  pub const fn to_timestamp(max_timestamp: u64) -> Self {
    Self { max_sequence_nr: u64::MAX, max_timestamp, min_sequence_nr: 0, min_timestamp: 0 }
  }

  /// Returns the maximum sequence number.
  #[must_use]
  pub const fn max_sequence_nr(&self) -> u64 {
    self.max_sequence_nr
  }

  /// Returns the maximum timestamp.
  #[must_use]
  pub const fn max_timestamp(&self) -> u64 {
    self.max_timestamp
  }

  /// Returns the minimum sequence number.
  #[must_use]
  pub const fn min_sequence_nr(&self) -> u64 {
    self.min_sequence_nr
  }

  /// Returns the minimum timestamp.
  #[must_use]
  pub const fn min_timestamp(&self) -> u64 {
    self.min_timestamp
  }

  /// Converts this typed criteria to the kernel snapshot selection contract.
  pub(crate) const fn to_kernel(&self) -> KernelSnapshotSelectionCriteria {
    KernelSnapshotSelectionCriteria::new(
      self.max_sequence_nr,
      self.max_timestamp,
      self.min_sequence_nr,
      self.min_timestamp,
    )
  }
}

impl Default for SnapshotSelectionCriteria {
  fn default() -> Self {
    Self::latest()
  }
}
