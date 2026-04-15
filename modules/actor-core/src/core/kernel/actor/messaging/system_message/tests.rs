use core::time::Duration;

use super::{FailurePayload, SystemMessage};
use crate::core::kernel::actor::{
  Pid,
  error::ActorError,
  messaging::{AnyMessage, Kill, PoisonPill},
};

#[test]
fn watch_message_round_trips_through_any_message() {
  let watcher = Pid::new(1, 0);
  let payload = SystemMessage::Watch(watcher);
  let stored: AnyMessage = payload.clone().into();
  let view = stored.as_view();
  let recovered = view.downcast_ref::<SystemMessage>().expect("system message");
  assert_eq!(recovered, &payload);
}

#[test]
fn create_message_round_trips_through_any_message() {
  let payload = SystemMessage::Create;
  let stored: AnyMessage = payload.clone().into();
  let view = stored.as_view();
  let recovered = view.downcast_ref::<SystemMessage>().expect("system message");
  assert_eq!(recovered, &payload);
}

#[test]
fn poison_pill_message_round_trips_through_any_message() {
  let payload = SystemMessage::PoisonPill;
  let stored: AnyMessage = payload.clone().into();
  let view = stored.as_view();
  let recovered = view.downcast_ref::<SystemMessage>().expect("system message");
  assert_eq!(recovered, &payload);
}

#[test]
fn poison_pill_public_message_converts_to_system_message() {
  // Given: 利用者が public auto-receive message を直接送る
  let payload = PoisonPill;

  // When: runtime 用の SystemMessage へ変換する
  let converted = SystemMessage::from(payload);

  // Then: 変換先は PoisonPill になる
  assert_eq!(converted, SystemMessage::PoisonPill);
}

#[test]
fn poison_pill_public_message_is_stored_as_distinct_payload_in_any_message() {
  // Given: 利用者が public auto-receive message を直接送る
  let stored = AnyMessage::new(PoisonPill);

  // When: AnyMessage から payload を参照する
  let view = stored.as_view();

  // Then: payload は alias 化されず public 型のまま保持される
  assert!(view.downcast_ref::<PoisonPill>().is_some());
  assert!(view.downcast_ref::<SystemMessage>().is_none());
}

#[test]
fn kill_message_round_trips_through_any_message() {
  let payload = SystemMessage::Kill;
  let stored: AnyMessage = payload.clone().into();
  let view = stored.as_view();
  let recovered = view.downcast_ref::<SystemMessage>().expect("system message");
  assert_eq!(recovered, &payload);
}

#[test]
fn kill_public_message_converts_to_system_message() {
  // Given: 利用者が public auto-receive message を直接送る
  let payload = Kill;

  // When: runtime 用の SystemMessage へ変換する
  let converted = SystemMessage::from(payload);

  // Then: 変換先は Kill になる
  assert_eq!(converted, SystemMessage::Kill);
}

#[test]
fn kill_public_message_is_stored_as_distinct_payload_in_any_message() {
  // Given: 利用者が public auto-receive message を直接送る
  let stored = AnyMessage::new(Kill);

  // When: AnyMessage から payload を参照する
  let view = stored.as_view();

  // Then: payload は alias 化されず public 型のまま保持される
  assert!(view.downcast_ref::<Kill>().is_some());
  assert!(view.downcast_ref::<SystemMessage>().is_none());
}

#[test]
fn recreate_message_round_trips_through_any_message() {
  let payload = SystemMessage::Recreate;
  let stored: AnyMessage = payload.clone().into();
  let view = stored.as_view();
  let recovered = view.downcast_ref::<SystemMessage>().expect("system message");
  assert_eq!(recovered, &payload);
}

#[test]
fn failure_message_round_trips_through_any_message() {
  let reason = ActorError::recoverable("boom");
  let failure = FailurePayload::from_error(Pid::new(5, 0), &reason, None, Duration::from_millis(1));
  let payload = SystemMessage::Failure(failure);
  let stored: AnyMessage = payload.clone().into();
  let view = stored.as_view();
  let recovered = view.downcast_ref::<SystemMessage>().expect("system message");
  assert_eq!(recovered, &payload);
}

#[test]
fn failure_payload_to_actor_error_preserves_classification() {
  let recoverable =
    FailurePayload::from_error(Pid::new(10, 0), &ActorError::recoverable("ok"), None, Duration::from_secs(1));
  let fatal = FailurePayload::from_error(Pid::new(11, 0), &ActorError::fatal("bad"), None, Duration::from_secs(2));

  if let ActorError::Recoverable(_) = recoverable.to_actor_error() {
  } else {
    panic!("expected recoverable");
  }

  if let ActorError::Fatal(_) = fatal.to_actor_error() {
  } else {
    panic!("expected fatal");
  }
}

#[test]
fn terminated_message_carries_pid() {
  let target = Pid::new(7, 0);
  if let SystemMessage::Terminated(pid) = SystemMessage::Terminated(target) {
    assert_eq!(pid, target);
  } else {
    panic!("unexpected variant");
  }
}
