#![cfg(test)]

use crate::{actor_ref::actor_ref_sender::ActorRefSender, any_message::AnyMessage, send_error::SendError};

struct TestSender;

impl ActorRefSender for TestSender {
  fn send(&self, _message: AnyMessage) -> Result<(), SendError> {
    Ok(())
  }
}

#[test]
fn trait_object_compile_check() {
  let sender = TestSender;
  assert!(sender.send(AnyMessage::new(1_u8)).is_ok());
}
