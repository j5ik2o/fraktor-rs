use alloc::{format, string::ToString};

use fraktor_persistence_core_kernel_rs::error::PersistenceError;

use crate::{EventRejectedError, PersistenceId};

#[test]
fn event_rejected_error_holds_persistence_id_sequence_number_and_cause() {
  let persistence_id = PersistenceId::of_unique_id("pid-rejected");
  let cause = PersistenceError::StateMachine("journal rejected event".to_string());
  let error = EventRejectedError::new(persistence_id.clone(), 42, cause.clone());
  assert_eq!(error.persistence_id(), &persistence_id);
  assert_eq!(error.sequence_nr(), 42);
  assert_eq!(error.cause(), &cause);
}

#[test]
fn event_rejected_error_display_includes_diagnostics() {
  let persistence_id = PersistenceId::of_unique_id("pid-display");
  let cause = PersistenceError::StateMachine("rejected by journal".to_string());
  let message = format!("{}", EventRejectedError::new(persistence_id, 9, cause));
  assert!(message.contains("pid-display"));
  assert!(message.contains("9"));
  assert!(message.contains("rejected by journal"));
}
