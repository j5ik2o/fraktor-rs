use fraktor_persistence_core_kernel_rs::state::DurableStateError;

use crate::{StateSourcedEffectorSignal, internal::StateSourcedStoreReply};

#[test]
fn recovery_completed_reply_converts_to_trusted_signal() {
  let signal =
    StateSourcedEffectorSignal::from(StateSourcedStoreReply::RecoveryCompleted { state: Some(42_u32), revision: 7 });

  assert!(matches!(signal, StateSourcedEffectorSignal::RecoveryCompleted {
    auth:     _,
    state:    Some(42),
    revision: 7,
  }));
}

#[test]
fn persisted_and_deleted_replies_convert_to_trusted_signals() {
  let persisted =
    StateSourcedEffectorSignal::from(StateSourcedStoreReply::StatePersisted { state: 10_u32, revision: 2 });
  let deleted = StateSourcedEffectorSignal::<u32>::from(StateSourcedStoreReply::StateDeleted { revision: 2 });

  assert!(matches!(persisted, StateSourcedEffectorSignal::StatePersisted { auth: _, state: 10, revision: 2 }));
  assert!(matches!(deleted, StateSourcedEffectorSignal::StateDeleted { auth: _, revision: 2 }));
}

#[test]
fn failed_replies_convert_to_failure_signals() {
  let recovery_error = DurableStateError::GetObjectFailed("load failed".into());
  let persistence_error = DurableStateError::UpsertObjectFailed("write failed".into());

  let recovery =
    StateSourcedEffectorSignal::<u32>::from(StateSourcedStoreReply::RecoveryFailed { error: recovery_error.clone() });
  let persistence = StateSourcedEffectorSignal::<u32>::from(StateSourcedStoreReply::PersistenceFailed {
    error: persistence_error.clone(),
  });

  assert!(matches!(recovery, StateSourcedEffectorSignal::RecoveryFailed { auth: _, error }
    if error == recovery_error));
  assert!(matches!(persistence, StateSourcedEffectorSignal::PersistenceFailed { auth: _, error }
    if error == persistence_error));
}
