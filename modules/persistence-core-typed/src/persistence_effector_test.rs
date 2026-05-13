use crate::{PersistenceEffector, RetentionCriteria};

#[test]
fn retention_delete_to_returns_none_for_zero_snapshot_interval() {
  let retention_criteria = RetentionCriteria::snapshot_every(0, 1);

  let actual = PersistenceEffector::<(), (), ()>::retention_delete_to(retention_criteria, 10);

  assert_eq!(actual, None);
}

#[test]
fn retention_delete_to_returns_none_for_zero_keep_snapshots() {
  let retention_criteria = RetentionCriteria::snapshot_every(2, 0);

  let actual = PersistenceEffector::<(), (), ()>::retention_delete_to(retention_criteria, 10);

  assert_eq!(actual, None);
}

#[test]
fn retention_delete_to_returns_none_before_first_snapshot_interval() {
  let retention_criteria = RetentionCriteria::snapshot_every(5, 1);

  let actual = PersistenceEffector::<(), (), ()>::retention_delete_to(retention_criteria, 3);

  assert_eq!(actual, None);
}
