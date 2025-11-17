use fraktor_utils_rs::core::{runtime_toolbox::NoStdToolbox, sync::ArcShared};

use crate::core::{
  actor_prim::{
    Pid,
    actor_ref::{ActorRef, ActorRefSender},
  },
  error::SendError,
  messaging::AnyMessage,
};

struct TestSender;

impl ActorRefSender<NoStdToolbox> for TestSender {
  fn send(&self, _message: AnyMessage) -> Result<(), SendError<NoStdToolbox>> {
    Ok(())
  }
}

#[test]
fn tell_delegates_to_sender() {
  let sender = ArcShared::new(TestSender);
  let pid = Pid::new(5, 1);
  let reference: ActorRef = ActorRef::new(pid, sender);
  assert!(reference.tell(AnyMessage::new("ping")).is_ok());
}

#[test]
fn null_sender_returns_error() {
  let reference: ActorRef = ActorRef::null();
  let error = reference.tell(AnyMessage::new("ping")).unwrap_err();
  assert!(matches!(error, SendError::Closed(_)));
}
