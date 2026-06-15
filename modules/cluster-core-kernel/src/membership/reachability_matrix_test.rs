use fraktor_remote_core_rs::address::{Address, UniqueAddress};

use crate::membership::{ReachabilityMatrix, ReachabilityStatus};

fn unique(host: &str, uid: u64) -> UniqueAddress {
  UniqueAddress::new(Address::new("cluster", host, 2552), uid)
}

#[test]
fn unreachable_update_stores_record_and_bumps_observer_version() {
  let observer = unique("observer-a", 1);
  let subject = unique("subject-a", 2);
  let mut matrix = ReachabilityMatrix::new();

  matrix.unreachable(observer.clone(), subject.clone());

  let snapshot = matrix.snapshot();
  assert_eq!(snapshot.records.len(), 1);
  assert_eq!(snapshot.records[0].observer, observer);
  assert_eq!(snapshot.records[0].subject, subject.clone());
  assert_eq!(snapshot.records[0].status, ReachabilityStatus::Unreachable);
  assert_eq!(snapshot.records[0].version, 1);
  assert_eq!(snapshot.observer_versions.get(&snapshot.records[0].observer), Some(&1));
  assert_eq!(matrix.aggregate_status(&subject), ReachabilityStatus::Unreachable);
}

#[test]
fn reachable_update_prunes_default_reachable_record_and_keeps_version() {
  let observer = unique("observer-a", 1);
  let subject = unique("subject-a", 2);
  let mut matrix = ReachabilityMatrix::new();

  matrix.unreachable(observer.clone(), subject.clone());
  matrix.reachable(observer.clone(), subject.clone());

  let snapshot = matrix.snapshot();
  assert!(snapshot.records.is_empty());
  assert_eq!(snapshot.observer_versions.get(&observer), Some(&2));
  assert_eq!(matrix.aggregate_status(&subject), ReachabilityStatus::Reachable);
}

#[test]
fn reachable_update_from_default_advances_observer_version_without_record() {
  let observer = unique("observer-a", 1);
  let subject = unique("subject-a", 2);
  let mut matrix = ReachabilityMatrix::new();

  matrix.reachable(observer.clone(), subject.clone());

  let snapshot = matrix.snapshot();
  assert!(snapshot.records.is_empty());
  assert_eq!(snapshot.observer_versions.get(&observer), Some(&1));
  assert_eq!(matrix.aggregate_status(&subject), ReachabilityStatus::Reachable);
}

#[test]
fn terminated_has_aggregate_precedence_and_is_not_overwritten_by_reachable() {
  let observer_a = unique("observer-a", 1);
  let observer_b = unique("observer-b", 2);
  let subject = unique("subject-a", 3);
  let mut matrix = ReachabilityMatrix::new();

  matrix.unreachable(observer_a.clone(), subject.clone());
  matrix.terminated(observer_b.clone(), subject.clone());
  matrix.reachable(observer_b.clone(), subject.clone());

  let snapshot = matrix.snapshot();
  let terminated = snapshot.records.iter().find(|record| record.observer == observer_b).expect("terminated record");
  assert_eq!(terminated.status, ReachabilityStatus::Terminated);
  assert_eq!(terminated.version, 1);
  assert_eq!(snapshot.observer_versions.get(&observer_b), Some(&1));
  assert_eq!(matrix.aggregate_status(&subject), ReachabilityStatus::Terminated);
}

#[test]
fn clear_subject_removes_terminated_records_and_bumps_observer_version() {
  let observer_a = unique("observer-a", 1);
  let observer_b = unique("observer-b", 2);
  let subject = unique("subject-a", 3);
  let other_subject = unique("subject-b", 4);
  let mut matrix = ReachabilityMatrix::new();

  matrix.terminated(observer_a.clone(), subject.clone());
  matrix.unreachable(observer_b.clone(), subject.clone());
  matrix.unreachable(observer_b.clone(), other_subject.clone());

  matrix.clear_subject(&subject);

  let snapshot = matrix.snapshot();
  assert!(snapshot.records.iter().all(|record| record.subject != subject));
  assert!(snapshot.records.iter().any(|record| record.subject == other_subject));
  assert_eq!(snapshot.observer_versions.get(&observer_a), Some(&2));
  assert_eq!(snapshot.observer_versions.get(&observer_b), Some(&3));
  assert_eq!(matrix.aggregate_status(&subject), ReachabilityStatus::Reachable);
}

#[test]
fn clear_observer_removes_row_and_bumps_observer_version_once() {
  let observer = unique("observer-a", 1);
  let subject_a = unique("subject-a", 2);
  let subject_b = unique("subject-b", 3);
  let other_observer = unique("observer-b", 4);
  let mut matrix = ReachabilityMatrix::new();

  matrix.unreachable(observer.clone(), subject_a.clone());
  matrix.terminated(observer.clone(), subject_b.clone());
  matrix.unreachable(other_observer.clone(), subject_b.clone());

  matrix.clear_observer(&observer);

  let snapshot = matrix.snapshot();
  assert!(snapshot.records.iter().all(|record| record.observer != observer));
  assert!(snapshot.records.iter().any(|record| record.observer == other_observer));
  assert_eq!(snapshot.observer_versions.get(&observer), Some(&3));
  assert_eq!(snapshot.observer_versions.get(&other_observer), Some(&1));
  assert_eq!(matrix.aggregate_status(&subject_a), ReachabilityStatus::Reachable);
  assert_eq!(matrix.aggregate_status(&subject_b), ReachabilityStatus::Unreachable);
}

#[test]
fn repeated_same_status_does_not_advance_observer_version() {
  let observer = unique("observer-a", 1);
  let subject = unique("subject-a", 2);
  let mut matrix = ReachabilityMatrix::new();

  matrix.unreachable(observer.clone(), subject.clone());
  matrix.unreachable(observer.clone(), subject);

  let snapshot = matrix.snapshot();
  assert_eq!(snapshot.records[0].version, 1);
  assert_eq!(snapshot.observer_versions.get(&observer), Some(&1));
}

#[test]
fn snapshot_preserves_matrix_records_and_observer_versions() {
  let observer_a = unique("observer-a", 1);
  let observer_b = unique("observer-b", 2);
  let subject_a = unique("subject-a", 3);
  let subject_b = unique("subject-b", 4);
  let mut matrix = ReachabilityMatrix::new();

  matrix.unreachable(observer_a.clone(), subject_a.clone());
  matrix.terminated(observer_b.clone(), subject_b.clone());

  let snapshot = matrix.snapshot();
  assert_eq!(snapshot.records.len(), 2);
  assert_eq!(snapshot.observer_versions.get(&observer_a), Some(&1));
  assert_eq!(snapshot.observer_versions.get(&observer_b), Some(&1));
  assert_eq!(matrix.aggregate_status(&subject_a), ReachabilityStatus::Unreachable);
  assert_eq!(matrix.aggregate_status(&subject_b), ReachabilityStatus::Terminated);
}
