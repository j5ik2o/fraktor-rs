use alloc::vec::Vec;
use core::time::Duration;

use fraktor_utils_rs::core::sync::{ArcShared, NoStdMutex};

use super::Behaviors;
use crate::core::{
  kernel::{
    actor::{
      ActorContext, Pid,
      actor_ref::{ActorRef, ActorRefSender, SendOutcome},
    },
    error::{ActorError, SendError},
    messaging::AnyMessage,
    system::ActorSystem,
  },
  typed::{
    actor::{TypedActorContext, TypedActorRef},
    behavior::Behavior,
    behavior_interceptor::BehaviorInterceptor,
    behavior_signal::BehaviorSignal,
    receive_timeout_config::ReceiveTimeoutConfig,
  },
};

struct Query(u32);

struct RecordingSender {
  inbox: ArcShared<NoStdMutex<Vec<AnyMessage>>>,
}

impl RecordingSender {
  fn new(inbox: ArcShared<NoStdMutex<Vec<AnyMessage>>>) -> Self {
    Self { inbox }
  }
}

impl ActorRefSender for RecordingSender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    self.inbox.lock().push(message);
    Ok(SendOutcome::Delivered)
  }
}

#[test]
fn receive_and_reply_sends_response_to_sender() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let inbox = ArcShared::new(NoStdMutex::new(Vec::new()));
  let sender = ActorRef::new(Pid::new(900, 0), RecordingSender::new(inbox.clone()));

  let mut context = ActorContext::new(&system, pid);
  context.set_sender(Some(sender));

  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);
  let mut behavior = Behaviors::receive_and_reply(|_ctx, message: &Query| Ok(message.0 + 1));
  behavior.handle_message(&mut typed_ctx, &Query(41)).expect("reply should succeed");

  let captured = inbox.lock();
  assert_eq!(captured.len(), 1);
  let value = captured[0].payload().downcast_ref::<u32>().expect("u32 reply");
  assert_eq!(*value, 42);
}

#[test]
fn receive_message_handles_message() {
  let received = ArcShared::new(NoStdMutex::new(Vec::new()));
  let received_clone = received.clone();

  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  let mut behavior = Behaviors::receive_message(move |_ctx, message: &u32| {
    received_clone.lock().push(*message);
    Ok(Behaviors::same())
  });
  behavior.handle_message(&mut typed_ctx, &42).expect("receive should delegate");

  assert_eq!(received.lock().as_slice(), &[42]);
}

#[test]
fn receive_and_reply_returns_recoverable_error_without_sender() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  let mut behavior = Behaviors::receive_and_reply(|_ctx, message: &Query| Ok(message.0 + 1));
  let result = behavior.handle_message(&mut typed_ctx, &Query(1));

  assert!(matches!(result, Err(ActorError::Recoverable(_))));
}

#[test]
fn with_timers_produces_active_behavior_with_signal_handler() {
  let behavior = Behaviors::with_timers::<u32, _>(|_timers| Behaviors::ignore());
  assert!(behavior.has_signal_handler());
}

#[test]
fn with_timers_shared_handle_usable_in_closures() {
  let behavior = Behaviors::with_timers::<u32, _>(|timers| {
    let timers_for_handler = timers.clone();
    Behaviors::receive_message(move |_ctx, _msg: &u32| {
      let key = crate::core::typed::timer_key::TimerKey::new("dynamic");
      assert!(!timers_for_handler.lock().is_timer_active(&key));
      Ok(Behaviors::same())
    })
  });
  assert!(behavior.has_signal_handler());
}

#[test]
fn receive_message_partial_returns_behavior_on_some() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  let mut behavior = Behaviors::receive_message_partial(
    |_ctx, msg: &u32| {
      if *msg > 10 { Ok(Some(Behaviors::same())) } else { Ok(None) }
    },
  );

  let result = behavior.handle_message(&mut typed_ctx, &20u32).expect("should handle");
  assert!(matches!(result.directive(), crate::core::typed::behavior::BehaviorDirective::Same));
}

#[test]
fn receive_message_partial_returns_unhandled_on_none() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  let mut behavior = Behaviors::receive_message_partial(
    |_ctx, msg: &u32| {
      if *msg > 10 { Ok(Some(Behaviors::same())) } else { Ok(None) }
    },
  );

  let result = behavior.handle_message(&mut typed_ctx, &5u32).expect("should return unhandled");
  assert!(matches!(result.directive(), crate::core::typed::behavior::BehaviorDirective::Unhandled));
}

#[test]
fn receive_partial_handles_message() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  let mut behavior =
    Behaviors::receive_partial(|_ctx, msg: &u32| if *msg == 42 { Ok(Some(Behaviors::same())) } else { Ok(None) });

  let result = behavior.handle_message(&mut typed_ctx, &42u32).expect("handled");
  assert!(matches!(result.directive(), crate::core::typed::behavior::BehaviorDirective::Same));

  let unhandled = behavior.handle_message(&mut typed_ctx, &7u32).expect("unhandled");
  assert!(matches!(unhandled.directive(), crate::core::typed::behavior::BehaviorDirective::Unhandled));
}

#[test]
fn receive_partial_chains_with_receive_signal() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  let mut behavior =
    Behaviors::receive_partial(|_ctx, msg: &u32| if *msg == 42 { Ok(Some(Behaviors::same())) } else { Ok(None) })
      .receive_signal(|_ctx, _signal| Ok(Behaviors::same()));

  assert!(behavior.has_signal_handler());
  let result = behavior.handle_message(&mut typed_ctx, &42u32).expect("handled");
  assert!(matches!(result.directive(), crate::core::typed::behavior::BehaviorDirective::Same));
}

struct RecordingInterceptor {
  receive_count: ArcShared<NoStdMutex<u32>>,
  start_count:   ArcShared<NoStdMutex<u32>>,
  signal_count:  ArcShared<NoStdMutex<u32>>,
}

impl BehaviorInterceptor<u32> for RecordingInterceptor {
  fn around_start(
    &mut self,
    ctx: &mut TypedActorContext<'_, u32>,
    start: &mut dyn FnMut(&mut TypedActorContext<'_, u32>) -> Result<Behavior<u32>, ActorError>,
  ) -> Result<Behavior<u32>, ActorError> {
    *self.start_count.lock() += 1;
    start(ctx)
  }

  fn around_receive(
    &mut self,
    ctx: &mut TypedActorContext<'_, u32>,
    message: &u32,
    target: &mut dyn FnMut(&mut TypedActorContext<'_, u32>, &u32) -> Result<Behavior<u32>, ActorError>,
  ) -> Result<Behavior<u32>, ActorError> {
    *self.receive_count.lock() += 1;
    target(ctx, message)
  }

  fn around_signal(
    &mut self,
    ctx: &mut TypedActorContext<'_, u32>,
    signal: &BehaviorSignal,
    target: &mut dyn FnMut(&mut TypedActorContext<'_, u32>, &BehaviorSignal) -> Result<Behavior<u32>, ActorError>,
  ) -> Result<Behavior<u32>, ActorError> {
    *self.signal_count.lock() += 1;
    target(ctx, signal)
  }
}

#[test]
fn intercept_delegates_started_to_interceptor() {
  let start_count = ArcShared::new(NoStdMutex::new(0u32));
  let start_count_clone = start_count.clone();
  let signal_count = ArcShared::new(NoStdMutex::new(0u32));
  let signal_count_clone = signal_count.clone();

  let mut behavior = Behaviors::intercept::<u32, _, _>(
    move || {
      Box::new(RecordingInterceptor {
        receive_count: ArcShared::new(NoStdMutex::new(0)),
        start_count:   start_count_clone.clone(),
        signal_count:  signal_count_clone.clone(),
      })
    },
    || Behaviors::receive_message(|_ctx, _msg: &u32| Ok(Behaviors::same())),
  );

  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  behavior.handle_signal(&mut typed_ctx, &BehaviorSignal::Started).expect("started");

  assert_eq!(*start_count.lock(), 1, "start interceptor should have been called once");
  assert_eq!(*signal_count.lock(), 0, "started should not be counted as a signal interception");
}

#[test]
fn intercept_delegates_message_to_interceptor() {
  let receive_count = ArcShared::new(NoStdMutex::new(0u32));
  let receive_count_clone = receive_count.clone();

  let mut behavior = Behaviors::intercept::<u32, _, _>(
    move || {
      Box::new(RecordingInterceptor {
        receive_count: receive_count_clone.clone(),
        start_count:   ArcShared::new(NoStdMutex::new(0)),
        signal_count:  ArcShared::new(NoStdMutex::new(0)),
      })
    },
    || Behaviors::receive_message(|_ctx, _msg: &u32| Ok(Behaviors::same())),
  );

  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  let mut inner = behavior.handle_signal(&mut typed_ctx, &BehaviorSignal::Started).expect("started");

  inner.handle_message(&mut typed_ctx, &42u32).expect("message");

  assert_eq!(*receive_count.lock(), 1, "interceptor should have been called once");
}

#[test]
fn intercept_delegates_signal_to_interceptor() {
  let signal_count = ArcShared::new(NoStdMutex::new(0u32));
  let signal_count_clone = signal_count.clone();

  let mut behavior = Behaviors::intercept::<u32, _, _>(
    move || {
      Box::new(RecordingInterceptor {
        receive_count: ArcShared::new(NoStdMutex::new(0)),
        start_count:   ArcShared::new(NoStdMutex::new(0)),
        signal_count:  signal_count_clone.clone(),
      })
    },
    || Behaviors::receive_message(|_ctx, _msg: &u32| Ok(Behaviors::same())),
  );

  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  let mut inner = behavior.handle_signal(&mut typed_ctx, &BehaviorSignal::Started).expect("started");

  inner.handle_signal(&mut typed_ctx, &BehaviorSignal::Stopped).expect("signal");

  assert_eq!(*signal_count.lock(), 1, "signal interceptor should have been called once");
}

#[test]
fn receive_timeout_config_stores_duration_and_produces_message() {
  let config = ReceiveTimeoutConfig::<u32>::new(Duration::from_millis(500), || 99u32);
  assert_eq!(config.duration, Duration::from_millis(500));
  assert_eq!(config.make_message(), 99);
}

#[test]
fn set_receive_timeout_configures_state() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);
  let mut timeout_state: Option<ReceiveTimeoutConfig<u32>> = None;

  {
    let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None).with_receive_timeout(&mut timeout_state);
    typed_ctx.set_receive_timeout(Duration::from_millis(200), || 42u32);
  }

  let config = timeout_state.as_ref().expect("timeout should be configured");
  assert_eq!(config.duration, Duration::from_millis(200));
  assert_eq!(config.make_message(), 42);
}

#[test]
fn cancel_receive_timeout_clears_state() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);
  let mut timeout_state: Option<ReceiveTimeoutConfig<u32>> = None;

  {
    let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None).with_receive_timeout(&mut timeout_state);
    typed_ctx.set_receive_timeout(Duration::from_millis(200), || 42u32);
    typed_ctx.cancel_receive_timeout();
  }

  assert!(timeout_state.is_none(), "timeout should be cleared after cancel");
}

#[test]
fn monitor_sends_clone_to_monitor_ref() {
  let monitor_inbox = ArcShared::new(NoStdMutex::new(Vec::new()));
  let monitor_sender = RecordingSender::new(monitor_inbox.clone());
  let monitor_actor_ref = ActorRef::new(Pid::new(800, 0), monitor_sender);
  let monitor_typed_ref = TypedActorRef::<u32>::from_untyped(monitor_actor_ref);

  let mut behavior =
    Behaviors::monitor(monitor_typed_ref, || Behaviors::receive_message(|_ctx, _msg: &u32| Ok(Behaviors::same())));

  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  let mut inner = behavior.handle_signal(&mut typed_ctx, &BehaviorSignal::Started).expect("started");

  inner.handle_message(&mut typed_ctx, &42u32).expect("message");

  let captured = monitor_inbox.lock();
  assert_eq!(captured.len(), 1, "monitor should have received one message");
  let value = captured[0].payload().downcast_ref::<u32>().expect("u32 clone");
  assert_eq!(*value, 42);
}

#[test]
fn monitor_passes_message_to_inner_behavior() {
  let inner_received = ArcShared::new(NoStdMutex::new(Vec::<u32>::new()));
  let inner_received_clone = inner_received.clone();

  let monitor_inbox = ArcShared::new(NoStdMutex::new(Vec::new()));
  let monitor_sender = RecordingSender::new(monitor_inbox.clone());
  let monitor_actor_ref = ActorRef::new(Pid::new(801, 0), monitor_sender);
  let monitor_typed_ref = TypedActorRef::<u32>::from_untyped(monitor_actor_ref);

  let mut behavior = Behaviors::monitor(monitor_typed_ref, move || {
    let received = inner_received_clone.clone();
    Behaviors::receive_message(move |_ctx, msg: &u32| {
      received.lock().push(*msg);
      Ok(Behaviors::same())
    })
  });

  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  let mut inner = behavior.handle_signal(&mut typed_ctx, &BehaviorSignal::Started).expect("started");
  inner.handle_message(&mut typed_ctx, &99u32).expect("message");

  let captured = inner_received.lock();
  assert_eq!(captured.len(), 1, "inner behavior should have received the message");
  assert_eq!(captured[0], 99);
}
