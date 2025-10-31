use alloc::{format, string::String, vec, vec::Vec};

use cellactor_utils_core_rs::sync::{ArcShared, sync_mutex_like::SpinSyncMutex};

use super::{MessageInvokerMiddleware, MessageInvokerPipeline};
use crate::{
  actor::Actor,
  actor_context::ActorContext,
  actor_error::ActorError,
  actor_ref::{ActorRef, ActorRefSender},
  any_message::{AnyMessage, AnyMessageView},
  pid::Pid,
  system::ActorSystem,
};

struct RecordingSender;

impl ActorRefSender for RecordingSender {
  fn send(&self, _message: AnyMessage) -> Result<(), crate::send_error::SendError> {
    Ok(())
  }
}

struct CaptureActor {
  payloads: SpinSyncMutex<Vec<u32>>,
  replies:  SpinSyncMutex<Vec<Option<ActorRef>>>,
}

impl CaptureActor {
  fn new() -> Self {
    Self { payloads: SpinSyncMutex::new(Vec::new()), replies: SpinSyncMutex::new(Vec::new()) }
  }

  fn payloads(&self) -> Vec<u32> {
    self.payloads.lock().clone()
  }

  fn replies(&self) -> Vec<Option<ActorRef>> {
    self.replies.lock().clone()
  }
}

impl Actor for CaptureActor {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(value) = message.downcast_ref::<u32>() {
      self.payloads.lock().push(*value);
    }
    self.replies.lock().push(ctx.reply_to().cloned());
    Ok(())
  }
}

struct LoggingActor {
  log: ArcShared<SpinSyncMutex<Vec<String>>>,
}

impl LoggingActor {
  fn new(log: ArcShared<SpinSyncMutex<Vec<String>>>) -> Self {
    Self { log }
  }

  fn record(&self, entry: &str) {
    self.log.lock().push(String::from(entry));
  }
}

impl Actor for LoggingActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    self.record("actor");
    Ok(())
  }
}

struct RecordingMiddleware {
  name: String,
  log:  ArcShared<SpinSyncMutex<Vec<String>>>,
}

impl RecordingMiddleware {
  fn new(name: &str, log: ArcShared<SpinSyncMutex<Vec<String>>>) -> Self {
    Self { name: String::from(name), log }
  }

  fn record(&self, suffix: &str) {
    self.log.lock().push(format!("{}:{}", self.name, suffix));
  }
}

impl MessageInvokerMiddleware for RecordingMiddleware {
  fn before_user(&self, _ctx: &mut ActorContext<'_>, _message: &AnyMessageView<'_>) -> Result<(), ActorError> {
    self.record("before");
    Ok(())
  }

  fn after_user(
    &self,
    _ctx: &mut ActorContext<'_>,
    _message: &AnyMessageView<'_>,
    result: Result<(), ActorError>,
  ) -> Result<(), ActorError> {
    self.record("after");
    result
  }
}

#[test]
fn pipeline_sets_and_clears_reply_to() {
  let system = ActorSystem::new_empty();
  let pid = Pid::new(1, 0);
  let mut ctx = ActorContext::new(&system, pid);
  let mut actor = CaptureActor::new();
  let pipeline = MessageInvokerPipeline::new();

  let reply_sender = ArcShared::new(RecordingSender);
  let reply_ref = ActorRef::new(Pid::new(2, 0), reply_sender);

  let message = AnyMessage::new(123_u32).with_reply_to(reply_ref.clone());
  pipeline.invoke_user(&mut actor, &mut ctx, message).expect("invoke user message");

  assert_eq!(actor.payloads(), vec![123_u32]);
  assert_eq!(actor.replies(), vec![Some(reply_ref)]);
  assert!(ctx.reply_to().is_none());
}

#[test]
fn pipeline_restores_previous_reply_target() {
  let system = ActorSystem::new_empty();
  let pid = Pid::new(10, 0);
  let mut ctx = ActorContext::new(&system, pid);
  let mut actor = CaptureActor::new();
  let pipeline = MessageInvokerPipeline::new();

  let previous_sender = ArcShared::new(RecordingSender);
  let previous_ref = ActorRef::new(Pid::new(3, 0), previous_sender);
  ctx.set_reply_to(Some(previous_ref.clone()));

  pipeline.invoke_user(&mut actor, &mut ctx, AnyMessage::new(7_u32)).expect("invoke");

  assert_eq!(actor.payloads(), vec![7_u32]);
  assert_eq!(actor.replies(), vec![None]);
  assert_eq!(ctx.reply_to(), Some(&previous_ref));
}

#[test]
fn middleware_executes_in_expected_order() {
  let system = ActorSystem::new_empty();
  let pid = Pid::new(42, 0);
  let mut ctx = ActorContext::new(&system, pid);
  let log = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let mut actor = LoggingActor::new(log.clone());

  let middleware_a: ArcShared<dyn MessageInvokerMiddleware> =
    ArcShared::new(RecordingMiddleware::new("a", log.clone()));
  let middleware_b: ArcShared<dyn MessageInvokerMiddleware> =
    ArcShared::new(RecordingMiddleware::new("b", log.clone()));
  let pipeline = MessageInvokerPipeline::from_middlewares(vec![middleware_a, middleware_b]);

  pipeline.invoke_user(&mut actor, &mut ctx, AnyMessage::new(1_u8)).expect("invoke");

  assert_eq!(log.lock().clone(), vec![
    String::from("a:before"),
    String::from("b:before"),
    String::from("actor"),
    String::from("b:after"),
    String::from("a:after"),
  ]);
}
