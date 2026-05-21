use crate::{Recovery, SnapshotSelectionCriteria};

#[test]
fn default_recovery_uses_latest_snapshot_and_unbounded_replay() {
  let recovery = Recovery::default();
  let kernel = recovery.to_kernel();

  assert_eq!(kernel.snapshot_criteria().max_sequence_nr(), u64::MAX);
  assert_eq!(kernel.to_sequence_nr(), u64::MAX);
  assert_eq!(kernel.replay_max(), u64::MAX);
}

#[test]
fn snapshot_disabled_recovery_replays_events_only() {
  let recovery = Recovery::without_snapshot();
  let kernel = recovery.to_kernel();

  assert_eq!(kernel.snapshot_criteria().max_sequence_nr(), 0);
  assert_eq!(kernel.to_sequence_nr(), u64::MAX);
  assert_eq!(kernel.replay_max(), u64::MAX);
}

#[test]
fn sequence_bound_snapshot_selection_is_preserved() {
  let recovery = Recovery::from_snapshot(SnapshotSelectionCriteria::to_sequence_nr(7));
  let kernel = recovery.to_kernel();

  assert_eq!(kernel.snapshot_criteria().max_sequence_nr(), 7);
}

#[test]
fn replay_bounds_are_preserved() {
  let recovery = Recovery::new(20, 5);
  let kernel = recovery.to_kernel();

  assert_eq!(kernel.to_sequence_nr(), 20);
  assert_eq!(kernel.replay_max(), 5);
}
