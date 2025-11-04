use cellactor_utils_core_rs::sync::ArcShared;

use crate::{
  NoStdToolbox,
  actor_prim::{
    Pid,
    actor_ref::{ActorRef, ActorRefGeneric, ActorRefSender},
  },
  error::SendError,
  messaging::{AnyMessage, AnyMessageGeneric},
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
  let reference: ActorRef = ActorRefGeneric::new(pid, sender);
  assert!(reference.tell(AnyMessageGeneric::new("ping")).is_ok());
}

#[test]
fn null_sender_returns_error() {
  let reference: ActorRef = ActorRefGeneric::null();
  let error = reference.tell(AnyMessageGeneric::new("ping")).unwrap_err();
  assert!(matches!(error, SendError::Closed(_)));
}
