use crate::core::{snapshot_metadata::SnapshotMetadata, snapshot_selection_criteria::SnapshotSelectionCriteria};

#[test]
fn snapshot_selection_criteria_latest_matches_any() {
  let criteria = SnapshotSelectionCriteria::latest();
  let metadata = SnapshotMetadata::new("user-1", 10, 50);

  assert!(criteria.matches(&metadata));
}

#[test]
fn snapshot_selection_criteria_none_matches_nothing() {
  let criteria = SnapshotSelectionCriteria::none();
  let metadata = SnapshotMetadata::new("user-1", 1, 1);

  assert!(!criteria.matches(&metadata));
}

#[test]
fn snapshot_selection_criteria_matches_range() {
  let criteria = SnapshotSelectionCriteria::new(10, 100, 5, 20);
  let within = SnapshotMetadata::new("user-1", 7, 50);
  let too_old = SnapshotMetadata::new("user-1", 4, 50);
  let too_new = SnapshotMetadata::new("user-1", 12, 50);
  let too_early = SnapshotMetadata::new("user-1", 7, 10);
  let too_late = SnapshotMetadata::new("user-1", 7, 120);

  assert!(criteria.matches(&within));
  assert!(!criteria.matches(&too_old));
  assert!(!criteria.matches(&too_new));
  assert!(!criteria.matches(&too_early));
  assert!(!criteria.matches(&too_late));
}

#[test]
fn snapshot_selection_criteria_limit_restricts_max_sequence() {
  let criteria = SnapshotSelectionCriteria::new(10, 100, 0, 0);
  let limited = criteria.limit(5);
  let allowed = SnapshotMetadata::new("user-1", 5, 10);
  let blocked = SnapshotMetadata::new("user-1", 6, 10);

  assert!(limited.matches(&allowed));
  assert!(!limited.matches(&blocked));
}
