use fraktor_persistence_core_kernel_rs::state::DurableStateError;
use fraktor_persistence_core_typed_rs::{DurableStateSignal, PersistenceEffectorSignal};

#[derive(Clone, Debug, PartialEq, Eq)]
enum PrivateMessage {
  Durable(DurableStateSignal<u32>),
}

#[test]
fn durable_state_signal_can_be_wrapped_by_user_private_message() {
  let signal = DurableStateSignal::RecoveryCompleted { state: Some(10_u32), revision: 3 };
  let message = PrivateMessage::Durable(signal);

  match message {
    | PrivateMessage::Durable(DurableStateSignal::RecoveryCompleted { state, revision }) => {
      assert_eq!(state, Some(10_u32));
      assert_eq!(revision, 3);
    },
    | PrivateMessage::Durable(_) => panic!("unexpected durable state signal"),
  }
}

#[test]
fn durable_state_persisted_signal_is_separate_from_event_sourced_signal() {
  let durable = DurableStateSignal::StatePersisted { state: 10_u32, revision: 4 };
  let event_sourced: Option<PersistenceEffectorSignal<u32, u32>> = None;

  assert!(matches!(durable, DurableStateSignal::StatePersisted { revision: 4, .. }));
  assert!(event_sourced.is_none());
}

#[test]
fn durable_state_failure_signals_carry_kernel_error_payloads() {
  let error = DurableStateError::GetObjectFailed("store unavailable".into());
  let recovery_failed = DurableStateSignal::<u32>::RecoveryFailed { error: error.clone() };
  let persistence_failed = DurableStateSignal::<u32>::PersistenceFailed { error };

  assert!(matches!(recovery_failed, DurableStateSignal::RecoveryFailed { .. }));
  assert!(matches!(persistence_failed, DurableStateSignal::PersistenceFailed { .. }));
}
