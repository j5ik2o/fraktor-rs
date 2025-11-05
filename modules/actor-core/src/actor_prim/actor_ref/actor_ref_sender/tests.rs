use super::*;
use crate::{error::SendError, messaging::AnyMessage};

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
