use crate::ddata::{Delete, DeleteResponse, DeleteWriteOutcome, Flag, FlagKey, ReplicatorEntry, WriteConsistency};

fn flag_key() -> FlagKey {
  FlagKey::new("flag")
}

#[test]
fn delete_present_entry_creates_tombstone_and_success_response() {
  let command = Delete::<Flag, u64>::new(flag_key(), WriteConsistency::Local).with_request(15);
  let entry = ReplicatorEntry::present(Flag::disabled().switch_on());

  let (next, response) = command.evaluate(&entry, DeleteWriteOutcome::Success);

  assert!(next.is_deleted());
  assert!(matches!(response, DeleteResponse::Success { .. }));
  assert!(response.is_locally_deleted());
  assert_eq!(response.request(), Some(&15));
}

#[test]
fn delete_missing_entry_still_creates_tombstone() {
  let command = Delete::<Flag>::new(flag_key(), WriteConsistency::Local);

  let (next, response) = command.evaluate(&ReplicatorEntry::missing(), DeleteWriteOutcome::ReplicationFailure);

  assert!(next.is_deleted());
  assert!(matches!(response, DeleteResponse::ReplicationFailure { .. }));
  assert!(response.is_locally_deleted());
}

#[test]
fn delete_already_deleted_entry_reports_data_deleted() {
  let command = Delete::<Flag>::new(flag_key(), WriteConsistency::Local);

  let (next, response) = command.evaluate(&ReplicatorEntry::deleted(), DeleteWriteOutcome::Success);

  assert!(next.is_deleted());
  assert!(matches!(response, DeleteResponse::DataDeleted { .. }));
  assert!(!response.is_locally_deleted());
}

#[test]
fn delete_store_failure_still_creates_local_tombstone() {
  let command = Delete::<Flag>::new(flag_key(), WriteConsistency::Local);

  let (next, response) = command.evaluate(&ReplicatorEntry::missing(), DeleteWriteOutcome::StoreFailure);

  assert!(next.is_deleted());
  assert!(matches!(response, DeleteResponse::StoreFailure { .. }));
  assert!(response.is_locally_deleted());
}
