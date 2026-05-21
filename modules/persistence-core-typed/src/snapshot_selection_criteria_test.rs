use fraktor_persistence_core_kernel_rs::snapshot::SnapshotMetadata;

use crate::SnapshotSelectionCriteria;

#[test]
fn latest_selects_unbounded_snapshot_range() {
  let criteria = SnapshotSelectionCriteria::latest();

  assert_eq!(criteria.max_sequence_nr(), u64::MAX);
  assert_eq!(criteria.max_timestamp(), u64::MAX);
  assert_eq!(criteria.min_sequence_nr(), 0);
  assert_eq!(criteria.min_timestamp(), 0);
}

#[test]
fn none_selects_no_snapshots() {
  let criteria = SnapshotSelectionCriteria::none();

  assert_eq!(criteria.max_sequence_nr(), 0);
  assert_eq!(criteria.max_timestamp(), 0);
  assert_eq!(criteria.min_sequence_nr(), 1);
  assert_eq!(criteria.min_timestamp(), 1);
}

#[test]
fn sequence_bound_translates_to_kernel_criteria() {
  let criteria = SnapshotSelectionCriteria::to_sequence_nr(42);
  let kernel = criteria.to_kernel();

  assert_eq!(kernel.max_sequence_nr(), 42);
}

#[test]
fn timestamp_bound_translates_to_kernel_criteria() {
  let criteria = SnapshotSelectionCriteria::to_timestamp(100);
  let kernel = criteria.to_kernel();

  assert!(kernel.matches(&SnapshotMetadata::new("pid", 10, 100)));
  assert!(!kernel.matches(&SnapshotMetadata::new("pid", 10, 101)));
}
