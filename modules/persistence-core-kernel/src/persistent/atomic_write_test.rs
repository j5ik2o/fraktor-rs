use alloc::string::ToString;

use fraktor_utils_core_rs::sync::ArcShared;

use crate::persistent::{AtomicWrite, AtomicWriteError, PersistentRepr};

fn repr(persistence_id: &str, sequence_nr: u64) -> PersistentRepr {
  PersistentRepr::new(persistence_id, sequence_nr, ArcShared::new(sequence_nr as i32))
}

#[test]
fn atomic_write_accepts_non_empty_payload_for_one_persistence_id() {
  let write = AtomicWrite::new(vec![repr("pid-1", 2), repr("pid-1", 3)]).expect("atomic write");

  assert_eq!(write.persistence_id(), "pid-1");
  assert_eq!(write.size(), 2);
  assert!(!write.is_empty());
  assert_eq!(write.payload().len(), 2);
}

#[test]
fn atomic_write_rejects_empty_payload() {
  let error = AtomicWrite::new(Vec::new()).expect_err("empty payload must fail");

  assert_eq!(error, AtomicWriteError::Empty);
}

#[test]
fn atomic_write_rejects_mixed_persistence_ids() {
  let error = AtomicWrite::new(vec![repr("pid-1", 1), repr("pid-2", 2)]).expect_err("mixed ids must fail");

  assert_eq!(error, AtomicWriteError::MixedPersistenceId { expected: "pid-1".into(), actual: "pid-2".into() });
}

#[test]
fn atomic_write_exposes_sequence_number_bounds() {
  let write = AtomicWrite::new(vec![repr("pid-1", 3), repr("pid-1", 1), repr("pid-1", 2)]).expect("atomic write");

  assert_eq!(write.lowest_sequence_nr(), 1);
  assert_eq!(write.highest_sequence_nr(), 3);
}

#[test]
fn atomic_write_consumes_payload() {
  let write = AtomicWrite::new(vec![repr("pid-1", 1)]).expect("atomic write");

  let payload = write.into_payload();

  assert_eq!(payload.len(), 1);
  assert_eq!(payload[0].persistence_id(), "pid-1");
}

#[test]
fn atomic_write_error_display_messages() {
  assert_eq!(AtomicWriteError::Empty.to_string(), "payload must not be empty");

  let error = AtomicWriteError::MixedPersistenceId { expected: "pid-1".into(), actual: "pid-2".into() };

  assert_eq!(error.to_string(), "mixed persistence id: expected \"pid-1\", actual \"pid-2\"");
}
