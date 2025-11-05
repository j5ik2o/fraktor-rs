use super::SystemMessage;
use crate::{actor_prim::Pid, messaging::AnyMessage};

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
fn terminated_message_carries_pid() {
  let target = Pid::new(7, 0);
  if let SystemMessage::Terminated(pid) = SystemMessage::Terminated(target) {
    assert_eq!(pid, target);
  } else {
    panic!("unexpected variant");
  }
}
