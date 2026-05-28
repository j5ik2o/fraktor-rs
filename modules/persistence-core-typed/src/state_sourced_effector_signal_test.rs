use fraktor_persistence_core_kernel_rs::state::DurableStateError;

use crate::{StateSourcedEffectorSignal, state_sourced_effector_signal_auth::StateSourcedEffectorSignalAuth};

#[test]
fn recovery_completed_carries_optional_state_and_revision() {
  let signal = StateSourcedEffectorSignal::RecoveryCompleted {
    auth:     StateSourcedEffectorSignalAuth::new(),
    state:    Some(42_u32),
    revision: 7,
  };

  assert!(
    matches!(signal, StateSourcedEffectorSignal::RecoveryCompleted { auth: _, state: Some(42), revision: 7 }),
    "recovery signal must carry durable state recovery result",
  );
}

#[test]
fn persisted_state_carries_state_and_revision() {
  let signal = StateSourcedEffectorSignal::StatePersisted {
    auth:     StateSourcedEffectorSignalAuth::new(),
    state:    42_u32,
    revision: 8,
  };

  assert!(
    matches!(signal, StateSourcedEffectorSignal::StatePersisted { auth: _, state: 42, revision: 8 }),
    "persisted signal must carry saved state and revision",
  );
}

#[test]
fn failure_signals_carry_kernel_durable_state_error() {
  let recovery_error = DurableStateError::GetObjectFailed("load failed".into());
  let persistence_error = DurableStateError::UpsertObjectFailed("write failed".into());
  let recovery_signal = StateSourcedEffectorSignal::<u32>::RecoveryFailed {
    auth:  StateSourcedEffectorSignalAuth::new(),
    error: recovery_error.clone(),
  };
  let persistence_signal = StateSourcedEffectorSignal::<u32>::PersistenceFailed {
    auth:  StateSourcedEffectorSignalAuth::new(),
    error: persistence_error.clone(),
  };

  assert!(
    matches!(
      recovery_signal,
      StateSourcedEffectorSignal::RecoveryFailed { auth: _, error } if error == recovery_error
    ),
    "recovery failure must preserve kernel durable state error",
  );
  assert!(
    matches!(
      persistence_signal,
      StateSourcedEffectorSignal::PersistenceFailed { auth: _, error } if error == persistence_error
    ),
    "persistence failure must preserve kernel durable state error",
  );
}
