use crate::ddata::{Flag, FlagKey, Get, GetResponse, ReadConsistency, ReplicatorEntry};

fn flag_key() -> FlagKey {
  FlagKey::new("flag")
}

#[test]
fn get_success_carries_present_data_and_request() {
  let command = Get::<Flag, u64>::new(flag_key(), ReadConsistency::Local).with_request(42);
  let entry = ReplicatorEntry::present(Flag::disabled().switch_on());

  let response = command.respond_from(&entry);

  assert!(response.is_success());
  assert_eq!(response.key().id(), "flag");
  assert_eq!(response.request(), Some(&42));
  assert!(response.data().expect("success response has data").is_enabled());
}

#[test]
fn get_missing_entry_returns_not_found() {
  let command = Get::<Flag>::new(flag_key(), ReadConsistency::Local);

  let response = command.respond_from(&ReplicatorEntry::missing());

  assert!(matches!(response, GetResponse::NotFound { .. }));
  assert_eq!(response.key().id(), "flag");
  assert!(response.data().is_none());
}

#[test]
fn get_deleted_entry_returns_data_deleted() {
  let command = Get::<Flag>::new(flag_key(), ReadConsistency::Local);

  let response = command.respond_from(&ReplicatorEntry::deleted());

  assert!(matches!(response, GetResponse::DataDeleted { .. }));
  assert_eq!(response.key().id(), "flag");
}

#[test]
fn get_failure_preserves_request_context() {
  let command = Get::<Flag, u64>::new(flag_key(), ReadConsistency::Local).with_request(7);

  let response = command.failure();

  assert!(matches!(response, GetResponse::Failure { .. }));
  assert_eq!(response.request(), Some(&7));
}
