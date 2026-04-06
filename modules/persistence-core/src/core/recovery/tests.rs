use crate::core::{recovery::Recovery, snapshot_selection_criteria::SnapshotSelectionCriteria};

#[test]
fn recovery_default_uses_latest() {
  let recovery = Recovery::default();

  assert_eq!(recovery.to_sequence_nr(), u64::MAX);
  assert_eq!(recovery.replay_max(), u64::MAX);
  assert_eq!(recovery.snapshot_criteria(), &SnapshotSelectionCriteria::latest());
}

#[test]
fn recovery_none_skips_replay() {
  let recovery = Recovery::none();

  assert_eq!(recovery.to_sequence_nr(), 0);
  assert_eq!(recovery.replay_max(), 0);
  assert_eq!(recovery.snapshot_criteria(), &SnapshotSelectionCriteria::none());
}

#[test]
fn recovery_new_sets_limits() {
  let recovery = Recovery::new(10, 5);

  assert_eq!(recovery.to_sequence_nr(), 10);
  assert_eq!(recovery.replay_max(), 5);
  assert_eq!(recovery.snapshot_criteria(), &SnapshotSelectionCriteria::latest());
}

#[test]
fn recovery_from_snapshot_sets_criteria() {
  let criteria = SnapshotSelectionCriteria::new(5, 100, 0, 0);
  let recovery = Recovery::from_snapshot(criteria.clone());

  assert_eq!(recovery.snapshot_criteria(), &criteria);
  assert_eq!(recovery.to_sequence_nr(), u64::MAX);
  assert_eq!(recovery.replay_max(), u64::MAX);
}
