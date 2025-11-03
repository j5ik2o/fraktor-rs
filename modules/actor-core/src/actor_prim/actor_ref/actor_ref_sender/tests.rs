use crate::{
  NoStdToolbox, actor_ref::actor_ref_sender::ActorRefSender, AnyMessage, SendError,
};

struct TestSender;

impl ActorRefSender<NoStdToolbox> for TestSender {
  fn send(&self, _message: AnyMessage<NoStdToolbox>) -> Result<(), SendError<NoStdToolbox>> {
    Ok(())
  }
}

#[test]
fn trait_object_compile_check() {
  let sender = TestSender;
  assert!(sender.send(AnyMessage::new(1_u8)).is_ok());
}
