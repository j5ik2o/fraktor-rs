use super::*;
use crate::core::actor_prim::{Pid, actor_ref::ActorRef};

#[test]
fn stores_payload_and_sender() {
  let mut message: AnyMessage = AnyMessage::new(5_u32);
  assert_eq!(message.payload().downcast_ref::<u32>(), Some(&5));

  let sender = ActorRef::null();
  message = message.with_sender(sender.clone());
  assert_eq!(message.sender(), Some(&sender));

  let view = message.as_view();
  assert_eq!(view.downcast_ref::<u32>(), Some(&5));
  assert!(view.sender().is_some());
  assert_eq!(view.sender().unwrap().pid(), Pid::new(0, 0));
}
