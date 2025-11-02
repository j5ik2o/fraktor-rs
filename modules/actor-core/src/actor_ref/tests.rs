#![cfg(test)]

use cellactor_utils_core_rs::sync::ArcShared;

use crate::{
  actor_ref::{actor_ref_sender::ActorRefSender, ActorRef},
  any_message::AnyMessage,
  pid::Pid,
  send_error::SendError,
};

struct TestSender;

impl ActorRefSender for TestSender {
  fn send(&self, _message: AnyMessage) -> Result<(), SendError> {
    Ok(())
  }
}

#[test]
fn tell_delegates_to_sender() {
  let sender = ArcShared::new(TestSender);
  let pid = Pid::new(5, 1);
  let reference = ActorRef::new(pid, sender);
  assert!(reference.tell(AnyMessage::new("ping")).is_ok());
}

#[test]
fn null_sender_returns_error() {
  let reference: ActorRef = ActorRef::null();
  let error = reference.tell(AnyMessage::new("ping")).unwrap_err();
  assert!(matches!(error, SendError::Closed(_)));
}
