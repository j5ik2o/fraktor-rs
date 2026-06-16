use alloc::string::String;

use crate::ddata::{Flag, FlagKey, ReplicatorEntry, Update, UpdateResponse, UpdateWriteOutcome, WriteConsistency};

fn flag_key() -> FlagKey {
  FlagKey::new("flag")
}

#[test]
fn update_missing_entry_passes_none_to_modifier_and_applies_success() {
  let command = Update::<Flag, u64>::new(flag_key(), WriteConsistency::Local).with_request(11);

  let (entry, response) = command.evaluate(
    &ReplicatorEntry::missing(),
    |current| {
      assert!(current.is_none());
      Ok(Flag::disabled().switch_on())
    },
    UpdateWriteOutcome::Success,
  );

  assert!(entry.data().expect("updated entry is present").is_enabled());
  assert!(matches!(response, UpdateResponse::Success { .. }));
  assert!(response.is_locally_applied());
  assert_eq!(response.request(), Some(&11));
}

#[test]
fn update_present_entry_passes_current_data_to_modifier() {
  let command = Update::<Flag>::new(flag_key(), WriteConsistency::Local);
  let existing = ReplicatorEntry::present(Flag::disabled());

  let (entry, response) = command.evaluate(
    &existing,
    |current| {
      assert!(!current.expect("present entry is visible").is_enabled());
      Ok(current.expect("present entry is visible").switch_on())
    },
    UpdateWriteOutcome::Timeout,
  );

  assert!(entry.data().expect("updated entry is present").is_enabled());
  assert!(matches!(response, UpdateResponse::Timeout { .. }));
  assert!(response.is_locally_applied());
}

#[test]
fn update_deleted_entry_rejects_without_calling_modifier() {
  let command = Update::<Flag>::new(flag_key(), WriteConsistency::Local);

  let (entry, response) = command.evaluate(
    &ReplicatorEntry::deleted(),
    |_| -> Result<Flag, String> { panic!("modifier must not be called for deleted entries") },
    UpdateWriteOutcome::Success,
  );

  assert!(entry.is_deleted());
  assert!(matches!(response, UpdateResponse::DataDeleted { .. }));
  assert!(!response.is_locally_applied());
}

#[test]
fn update_modify_failure_keeps_original_entry() {
  let command = Update::<Flag>::new(flag_key(), WriteConsistency::Local);
  let existing = ReplicatorEntry::present(Flag::disabled());

  let (entry, response) =
    command.evaluate(&existing, |_| Err(String::from("rejected by modifier")), UpdateWriteOutcome::Success);

  assert_eq!(entry, existing);
  assert!(matches!(response, UpdateResponse::ModifyFailure { .. }));
  assert_eq!(response.message(), Some("rejected by modifier"));
  assert!(!response.is_locally_applied());
}

#[test]
fn update_store_failure_still_applies_local_entry() {
  let command = Update::<Flag>::new(flag_key(), WriteConsistency::Local);

  let (entry, response) = command.evaluate(
    &ReplicatorEntry::missing(),
    |_| Ok(Flag::disabled().switch_on()),
    UpdateWriteOutcome::StoreFailure,
  );

  assert!(entry.data().expect("updated entry is present").is_enabled());
  assert!(matches!(response, UpdateResponse::StoreFailure { .. }));
  assert!(response.is_locally_applied());
}
