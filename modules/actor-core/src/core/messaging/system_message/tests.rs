use core::time::Duration;

use super::{FailurePayload, SystemMessage};
use crate::core::{actor_prim::Pid, error::ActorError, messaging::AnyMessage};

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
