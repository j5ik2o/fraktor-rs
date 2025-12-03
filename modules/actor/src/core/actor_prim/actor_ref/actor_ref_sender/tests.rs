use super::*;
use crate::core::{error::SendError, messaging::AnyMessage};

struct TestSender;

impl ActorRefSender for TestSender {
  fn send(&mut self, _message: AnyMessage) -> Result<crate::core::actor_prim::actor_ref::SendOutcome, SendError> {
    Ok(crate::core::actor_prim::actor_ref::SendOutcome::Delivered)
  }
}

#[test]
fn trait_object_compile_check() {
  let mut sender = TestSender;
  assert!(sender.send(AnyMessage::new(1_u8)).is_ok());
}
