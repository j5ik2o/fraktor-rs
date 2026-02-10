use alloc::vec::Vec;

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdMutex, NoStdToolbox},
  sync::ArcShared,
};

use super::Behaviors;
use crate::core::{
  actor::{
    ActorContextGeneric, Pid,
    actor_ref::{ActorRefGeneric, ActorRefSender, SendOutcome},
  },
  error::{ActorError, SendError},
  messaging::AnyMessageGeneric,
  system::ActorSystemGeneric,
  typed::actor::TypedActorContextGeneric,
};

struct Query(u32);

struct RecordingSender {
  inbox: ArcShared<NoStdMutex<Vec<AnyMessageGeneric<NoStdToolbox>>>>,
}

impl RecordingSender {
  fn new(inbox: ArcShared<NoStdMutex<Vec<AnyMessageGeneric<NoStdToolbox>>>>) -> Self {
    Self { inbox }
  }
}

impl ActorRefSender<NoStdToolbox> for RecordingSender {
  fn send(&mut self, message: AnyMessageGeneric<NoStdToolbox>) -> Result<SendOutcome, SendError<NoStdToolbox>> {
    self.inbox.lock().push(message);
    Ok(SendOutcome::Delivered)
  }
}

#[test]
fn receive_and_reply_sends_response_to_sender() {
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
  let pid = system.allocate_pid();
  let inbox = ArcShared::new(NoStdMutex::new(Vec::new()));
  let sender = ActorRefGeneric::new(Pid::new(900, 0), RecordingSender::new(inbox.clone()));

  let mut context = ActorContextGeneric::new(&system, pid);
  context.set_sender(Some(sender));

  let mut typed_ctx = TypedActorContextGeneric::from_untyped(&mut context, None);
  let mut behavior = Behaviors::receive_and_reply(|_ctx, message: &Query| Ok(message.0 + 1));
  let _ = behavior.handle_message(&mut typed_ctx, &Query(41)).expect("reply should succeed");

  let captured = inbox.lock();
  assert_eq!(captured.len(), 1);
  let value = captured[0].payload().downcast_ref::<u32>().expect("u32 reply");
  assert_eq!(*value, 42);
}

#[test]
fn receive_and_reply_returns_recoverable_error_without_sender() {
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContextGeneric::new(&system, pid);
  let mut typed_ctx = TypedActorContextGeneric::from_untyped(&mut context, None);

  let mut behavior = Behaviors::receive_and_reply(|_ctx, message: &Query| Ok(message.0 + 1));
  let result = behavior.handle_message(&mut typed_ctx, &Query(1));

  assert!(matches!(result, Err(ActorError::Recoverable(_))));
}
