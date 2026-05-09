use alloc::string::String;

use super::FsmTimerFired;
use crate::actor::messaging::AnyMessage;

#[test]
fn new_stores_name() {
  let fired = FsmTimerFired::new(String::from("tick"), 1, AnyMessage::new(42_u32));

  assert_eq!(fired.name(), "tick");
}

#[test]
fn new_stores_generation() {
  let fired = FsmTimerFired::new(String::from("tick"), 7, AnyMessage::new(42_u32));

  assert_eq!(fired.generation(), 7);
}

#[test]
fn new_stores_payload() {
  let fired = FsmTimerFired::new(String::from("tick"), 1, AnyMessage::new(42_u32));

  assert_eq!(fired.payload().downcast_ref::<u32>(), Some(&42));
}
