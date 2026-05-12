extern crate alloc;

use alloc::{format, string::String, vec, vec::Vec};

use fraktor_utils_core_rs::sync::{ArcShared, SpinSyncMutex};

use super::{MessageInvokerMiddleware, MessageInvokerPipeline, middleware_shared::MiddlewareShared};
use crate::{
  actor::{
    Actor, ActorContext, Pid,
    actor_ref::{ActorRef, ActorRefSender, SendOutcome},
    error::{ActorError, SendError},
    invoke_guard::{InvokeGuard, InvokeGuardFactory, NoopInvokeGuardFactory},
    messaging::{AnyMessage, AnyMessageView},
  },
  system::ActorSystem,
};

fn noop_pipeline() -> MessageInvokerPipeline {
  MessageInvokerPipeline::new_with_guard(NoopInvokeGuardFactory::new().build())
}

struct RecordingSender;

impl ActorRefSender for RecordingSender {
  fn send(&mut self, _message: AnyMessage) -> Result<SendOutcome, SendError> {
    Ok(SendOutcome::Delivered)
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
    self.replies.lock().push(ctx.sender().cloned());
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

struct SkippingInvokeGuard;

impl InvokeGuard for SkippingInvokeGuard {
  fn wrap_receive(&self, _call: &mut dyn FnMut() -> Result<(), ActorError>) -> Result<(), ActorError> {
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
  fn before_user(&mut self, _ctx: &mut ActorContext<'_>, _message: &AnyMessageView<'_>) -> Result<(), ActorError> {
    self.record("before");
    Ok(())
  }

  fn after_user(
    &mut self,
    _ctx: &mut ActorContext<'_>,
    _message: &AnyMessageView<'_>,
    result: Result<(), ActorError>,
  ) -> Result<(), ActorError> {
    self.record("after");
    result
  }
}

#[test]
fn pipeline_sets_and_clears_sender() {
  let system = ActorSystem::new_empty();
  let pid = Pid::new(1, 0);
  let mut ctx = ActorContext::new(&system, pid);
  let mut actor = CaptureActor::new();
  let pipeline = noop_pipeline();

  let reply_sender = RecordingSender;
  let reply_ref = ActorRef::new_with_builtin_lock(Pid::new(2, 0), reply_sender);

  let message = AnyMessage::new(123_u32).with_sender(reply_ref.clone());
  pipeline.invoke_user(&mut actor, &mut ctx, message).expect("invoke user message");

  assert_eq!(actor.payloads(), vec![123_u32]);
  assert_eq!(actor.replies(), vec![Some(reply_ref)]);
  assert!(ctx.sender().is_none());
}

#[test]
fn pipeline_restores_previous_sender() {
  let system = ActorSystem::new_empty();
  let pid = Pid::new(10, 0);
  let mut ctx = ActorContext::new(&system, pid);
  let mut actor = CaptureActor::new();
  let pipeline = noop_pipeline();

  let previous_sender = RecordingSender;
  let previous_ref = ActorRef::new_with_builtin_lock(Pid::new(3, 0), previous_sender);
  ctx.set_sender(Some(previous_ref.clone()));

  pipeline.invoke_user(&mut actor, &mut ctx, AnyMessage::new(7_u32)).expect("invoke");

  assert_eq!(actor.payloads(), vec![7_u32]);
  assert_eq!(actor.replies(), vec![None]);
  assert_eq!(ctx.sender(), Some(&previous_ref));
}

#[test]
fn middleware_executes_in_expected_order() {
  let system = ActorSystem::new_empty();
  let pid = Pid::new(42, 0);
  let mut ctx = ActorContext::new(&system, pid);
  let log = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let mut actor = LoggingActor::new(log.clone());

  let middleware_a =
    MiddlewareShared::new(Box::new(RecordingMiddleware::new("a", log.clone())) as Box<dyn MessageInvokerMiddleware>);
  let middleware_b =
    MiddlewareShared::new(Box::new(RecordingMiddleware::new("b", log.clone())) as Box<dyn MessageInvokerMiddleware>);
  let pipeline =
    MessageInvokerPipeline::from_middlewares(vec![middleware_a, middleware_b], NoopInvokeGuardFactory::new().build());

  pipeline.invoke_user(&mut actor, &mut ctx, AnyMessage::new(1_u8)).expect("invoke");

  assert_eq!(log.lock().clone(), vec![
    String::from("a:before"),
    String::from("b:before"),
    String::from("actor"),
    String::from("b:after"),
    String::from("a:after"),
  ]);
}

#[test]
fn pipeline_fails_when_guard_does_not_call_receive() {
  let system = ActorSystem::new_empty();
  let pid = Pid::new(50, 0);
  let mut ctx = ActorContext::new(&system, pid);
  let mut actor = CaptureActor::new();
  let guard: ArcShared<dyn InvokeGuard> = ArcShared::new(SkippingInvokeGuard);
  let pipeline = MessageInvokerPipeline::new_with_guard(guard);

  let result = pipeline.invoke_user(&mut actor, &mut ctx, AnyMessage::new(99_u32));

  assert!(matches!(result, Err(ActorError::Fatal(reason)) if reason.as_str() == "invoke guard did not call receive"));
  assert!(actor.payloads().is_empty());
  assert!(actor.replies().is_empty());
  assert!(ctx.sender().is_none());
}
