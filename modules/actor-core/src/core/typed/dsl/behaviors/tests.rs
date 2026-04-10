use alloc::{borrow::ToOwned, collections::BTreeMap, string::String, vec::Vec};
use core::time::Duration;
use std::sync::{Arc, Mutex, Once};

use fraktor_utils_core_rs::core::sync::{ArcShared, SpinSyncMutex};
use tracing::{
  Event, Level, Metadata, Subscriber,
  field::{Field, Visit},
  metadata::LevelFilter,
  span::{Attributes, Id, Record},
  subscriber::{Interest, with_default},
};

use super::Behaviors;
use crate::core::{
  kernel::{
    actor::{
      Actor, ActorCell, ActorContext, Pid,
      actor_ref::{ActorRef, ActorRefSender, SendOutcome},
      error::{ActorError, SendError},
      messaging::{AnyMessage, AnyMessageView},
      props::Props,
    },
    event::logging::LogLevel,
    system::ActorSystem,
  },
  typed::{
    LogOptions, TypedActorRef,
    actor::TypedActorContext,
    behavior::Behavior,
    behavior_interceptor::BehaviorInterceptor,
    dsl::TimerKey,
    internal::ReceiveTimeoutConfig,
    message_and_signals::{BehaviorSignal, PostStop, PreRestart},
  },
};

struct Query(u32);

struct TestActor;

impl Actor for TestActor {
  fn receive(&mut self, _context: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

struct RecordingSender {
  inbox: ArcShared<SpinSyncMutex<Vec<AnyMessage>>>,
}

impl RecordingSender {
  fn new(inbox: ArcShared<SpinSyncMutex<Vec<AnyMessage>>>) -> Self {
    Self { inbox }
  }
}

impl ActorRefSender for RecordingSender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    self.inbox.lock().push(message);
    Ok(SendOutcome::Delivered)
  }
}

fn register_cell(system: &ActorSystem, pid: Pid, name: &str, props: &Props) -> ArcShared<ActorCell> {
  let cell = ActorCell::create(system.state(), pid, None, String::from(name), props).expect("create actor cell");
  system.state().register_cell(cell.clone());
  cell
}

#[test]
fn receive_and_reply_sends_response_to_sender() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let inbox = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let sender = ActorRef::new_with_builtin_lock(Pid::new(900, 0), RecordingSender::new(inbox.clone()));

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
  let received = ArcShared::new(SpinSyncMutex::new(Vec::new()));
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
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let props = Props::from_fn(|| TestActor);
  let _cell = register_cell(&system, pid, "with-timers-signal", &props);
  let mut context = ActorContext::new(&system, pid);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  let mut behavior = Behaviors::with_timers::<u32, _>(|_timers| Behaviors::ignore());
  let active = behavior.handle_start(&mut typed_ctx).expect("with_timers should start");

  assert!(active.has_signal_handler());
}

#[test]
fn with_timers_shared_handle_usable_in_closures() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let props = Props::from_fn(|| TestActor);
  let _cell = register_cell(&system, pid, "with-timers-closure", &props);
  let mut context = ActorContext::new(&system, pid);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  let mut behavior = Behaviors::with_timers::<u32, _>(|timers| {
    let timers_for_handler = timers.clone();
    Behaviors::receive_message(move |_ctx, _msg: &u32| {
      let key = TimerKey::new("dynamic");
      assert!(!timers_for_handler.with_lock(|timers| timers.is_timer_active(&key)));
      Ok(Behaviors::same())
    })
  });
  let mut active = behavior.handle_start(&mut typed_ctx).expect("with_timers should start");

  assert!(active.has_signal_handler());
  let _ = active.handle_message(&mut typed_ctx, &1_u32).expect("closure should access shared timers");
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
  receive_count: ArcShared<SpinSyncMutex<u32>>,
  start_count:   ArcShared<SpinSyncMutex<u32>>,
  signal_count:  ArcShared<SpinSyncMutex<u32>>,
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
  let start_count = ArcShared::new(SpinSyncMutex::new(0u32));
  let start_count_clone = start_count.clone();
  let signal_count = ArcShared::new(SpinSyncMutex::new(0u32));
  let signal_count_clone = signal_count.clone();

  let mut behavior = Behaviors::intercept::<u32, _, _>(
    move || {
      Box::new(RecordingInterceptor {
        receive_count: ArcShared::new(SpinSyncMutex::new(0)),
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

  behavior.handle_start(&mut typed_ctx).expect("started");

  assert_eq!(*start_count.lock(), 1, "start interceptor should have been called once");
  assert_eq!(*signal_count.lock(), 0, "started should not be counted as a signal interception");
}

#[test]
fn intercept_delegates_message_to_interceptor() {
  let receive_count = ArcShared::new(SpinSyncMutex::new(0u32));
  let receive_count_clone = receive_count.clone();

  let mut behavior = Behaviors::intercept::<u32, _, _>(
    move || {
      Box::new(RecordingInterceptor {
        receive_count: receive_count_clone.clone(),
        start_count:   ArcShared::new(SpinSyncMutex::new(0)),
        signal_count:  ArcShared::new(SpinSyncMutex::new(0)),
      })
    },
    || Behaviors::receive_message(|_ctx, _msg: &u32| Ok(Behaviors::same())),
  );

  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  let mut inner = behavior.handle_start(&mut typed_ctx).expect("started");

  inner.handle_message(&mut typed_ctx, &42u32).expect("message");

  assert_eq!(*receive_count.lock(), 1, "interceptor should have been called once");
}

#[test]
fn intercept_delegates_signal_to_interceptor() {
  let signal_count = ArcShared::new(SpinSyncMutex::new(0u32));
  let signal_count_clone = signal_count.clone();

  let mut behavior = Behaviors::intercept::<u32, _, _>(
    move || {
      Box::new(RecordingInterceptor {
        receive_count: ArcShared::new(SpinSyncMutex::new(0)),
        start_count:   ArcShared::new(SpinSyncMutex::new(0)),
        signal_count:  signal_count_clone.clone(),
      })
    },
    || Behaviors::receive_message(|_ctx, _msg: &u32| Ok(Behaviors::same())),
  );

  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  let mut inner = behavior.handle_start(&mut typed_ctx).expect("started");

  inner.handle_signal(&mut typed_ctx, &BehaviorSignal::PostStop).expect("signal");

  assert_eq!(*signal_count.lock(), 1, "signal interceptor should have been called once");
}

#[test]
fn intercept_behavior_clone_restarts_with_fresh_inner_behavior() {
  let start_count = ArcShared::new(SpinSyncMutex::new(0u32));
  let start_count_clone = start_count.clone();
  let receive_count = ArcShared::new(SpinSyncMutex::new(0u32));
  let receive_count_clone = receive_count.clone();

  let inner = Behaviors::setup(move |_ctx| {
    *start_count_clone.lock() += 1;
    Behaviors::receive_message(|_ctx, _msg: &u32| Ok(Behaviors::same()))
  });

  let behavior = Behaviors::intercept_behavior::<u32, _>(
    move || {
      Box::new(RecordingInterceptor {
        receive_count: receive_count_clone.clone(),
        start_count:   ArcShared::new(SpinSyncMutex::new(0)),
        signal_count:  ArcShared::new(SpinSyncMutex::new(0)),
      })
    },
    inner,
  );

  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  let mut first = behavior.clone().handle_start(&mut typed_ctx).expect("first started");
  let mut second = behavior.clone().handle_start(&mut typed_ctx).expect("second started");

  first.handle_message(&mut typed_ctx, &1u32).expect("first message");
  second.handle_message(&mut typed_ctx, &2u32).expect("second message");

  assert_eq!(*start_count.lock(), 2, "intercepted behavior should recreate the wrapped behavior for each clone");
  assert_eq!(*receive_count.lock(), 2, "each clone should invoke its own interceptor pipeline");
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
  let monitor_inbox = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let monitor_sender = RecordingSender::new(monitor_inbox.clone());
  let monitor_actor_ref = ActorRef::new_with_builtin_lock(Pid::new(800, 0), monitor_sender);
  let monitor_typed_ref = TypedActorRef::<u32>::from_untyped(monitor_actor_ref);

  let mut behavior =
    Behaviors::monitor(monitor_typed_ref, || Behaviors::receive_message(|_ctx, _msg: &u32| Ok(Behaviors::same())));

  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  let mut inner = behavior.handle_start(&mut typed_ctx).expect("started");

  inner.handle_message(&mut typed_ctx, &42u32).expect("message");

  let captured = monitor_inbox.lock();
  assert_eq!(captured.len(), 1, "monitor should have received one message");
  let value = captured[0].payload().downcast_ref::<u32>().expect("u32 clone");
  assert_eq!(*value, 42);
}

#[test]
fn monitor_passes_message_to_inner_behavior() {
  let inner_received = ArcShared::new(SpinSyncMutex::new(Vec::<u32>::new()));
  let inner_received_clone = inner_received.clone();

  let monitor_inbox = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let monitor_sender = RecordingSender::new(monitor_inbox.clone());
  let monitor_actor_ref = ActorRef::new_with_builtin_lock(Pid::new(801, 0), monitor_sender);
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

  let mut inner = behavior.handle_start(&mut typed_ctx).expect("started");
  inner.handle_message(&mut typed_ctx, &99u32).expect("message");

  let captured = inner_received.lock();
  assert_eq!(captured.len(), 1, "inner behavior should have received the message");
  assert_eq!(captured[0], 99);
}

// --- Phase 1 タスク1: receive_message_with_same ---

/// `receive_message_with_same` invokes the handler and returns `Same` directive.
#[test]
fn receive_message_with_same_invokes_handler_and_returns_same() {
  let received = ArcShared::new(SpinSyncMutex::new(Vec::<u32>::new()));
  let received_clone = received.clone();

  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  let mut behavior = Behaviors::receive_message_with_same(move |_ctx, message: &u32| {
    received_clone.lock().push(*message);
  });

  let result = behavior.handle_message(&mut typed_ctx, &42u32).expect("should handle message");
  assert!(
    matches!(result.directive(), crate::core::typed::behavior::BehaviorDirective::Same),
    "should return Same directive"
  );
  assert_eq!(received.lock().as_slice(), &[42], "handler should have received the message");
}

/// `receive_message_with_same` handles multiple messages with the same behavior.
#[test]
fn receive_message_with_same_handles_multiple_messages() {
  let received = ArcShared::new(SpinSyncMutex::new(Vec::<u32>::new()));
  let received_clone = received.clone();

  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  let mut behavior = Behaviors::receive_message_with_same(move |_ctx, message: &u32| {
    received_clone.lock().push(*message);
  });

  behavior.handle_message(&mut typed_ctx, &1u32).expect("first message");
  behavior.handle_message(&mut typed_ctx, &2u32).expect("second message");
  behavior.handle_message(&mut typed_ctx, &3u32).expect("third message");

  assert_eq!(received.lock().as_slice(), &[1, 2, 3]);
}

/// `receive_message_with_same` provides context access to the handler.
#[test]
fn receive_message_with_same_provides_context() {
  let captured_pid = ArcShared::new(SpinSyncMutex::new(0u64));
  let captured_pid_clone = captured_pid.clone();

  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  let mut behavior = Behaviors::receive_message_with_same(move |ctx, _message: &u32| {
    *captured_pid_clone.lock() = ctx.pid().value();
  });

  behavior.handle_message(&mut typed_ctx, &1u32).expect("should handle");
  assert_eq!(*captured_pid.lock(), typed_ctx.pid().value());
}

// --- Phase 1 タスク2: stopped_with_post_stop ---

/// `stopped_with_post_stop` has `Stopped` directive so the runner stops the actor immediately.
#[test]
fn stopped_with_post_stop_has_stopped_directive() {
  let behavior = Behaviors::stopped_with_post_stop::<u32, _>(|| {});
  assert!(
    matches!(behavior.directive(), crate::core::typed::behavior::BehaviorDirective::Stopped),
    "stopped_with_post_stop must have Stopped directive so the behavior runner stops the actor"
  );
}

/// `stopped_with_post_stop` executes the callback when `PostStop` signal is received.
#[test]
fn stopped_with_post_stop_executes_callback_on_post_stop_signal() {
  let called = ArcShared::new(SpinSyncMutex::new(false));
  let called_clone = called.clone();

  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  let mut behavior = Behaviors::stopped_with_post_stop::<u32, _>(move || {
    *called_clone.lock() = true;
  });

  behavior.handle_signal(&mut typed_ctx, &BehaviorSignal::PostStop).expect("should handle PostStop signal");

  assert!(*called.lock(), "post_stop callback should have been invoked");
}

/// `stopped_with_post_stop` returns `Stopped` directive after callback execution.
#[test]
fn stopped_with_post_stop_returns_stopped_directive() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  let mut behavior = Behaviors::stopped_with_post_stop::<u32, _>(|| {});

  let result = behavior.handle_signal(&mut typed_ctx, &BehaviorSignal::PostStop).expect("should handle");
  assert!(
    matches!(result.directive(), crate::core::typed::behavior::BehaviorDirective::Stopped),
    "should return Stopped directive"
  );
}

/// `stopped_with_post_stop` does not invoke callback for non-PostStop signals.
#[test]
fn stopped_with_post_stop_ignores_non_post_stop_signals() {
  let called = ArcShared::new(SpinSyncMutex::new(false));
  let called_clone = called.clone();

  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  let mut behavior = Behaviors::stopped_with_post_stop::<u32, _>(move || {
    *called_clone.lock() = true;
  });

  let result = behavior.handle_signal(&mut typed_ctx, &BehaviorSignal::PreRestart).expect("should handle PreRestart");
  assert!(
    matches!(result.directive(), crate::core::typed::behavior::BehaviorDirective::Same),
    "non-PostStop signals should return Same"
  );
  assert!(!*called.lock(), "callback should not be invoked for PreRestart signal");
}

#[test]
fn stopped_with_post_stop_accepts_public_post_stop_conversion() {
  let called = ArcShared::new(SpinSyncMutex::new(false));
  let called_clone = called.clone();

  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  let mut behavior = Behaviors::stopped_with_post_stop::<u32, _>(move || {
    *called_clone.lock() = true;
  });

  behavior.handle_signal(&mut typed_ctx, &BehaviorSignal::from(PostStop)).expect("should handle PostStop signal");

  assert!(*called.lock(), "post_stop callback should have been invoked");
}

#[test]
fn stopped_with_post_stop_ignores_public_pre_restart_conversion() {
  let called = ArcShared::new(SpinSyncMutex::new(false));
  let called_clone = called.clone();

  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  let mut behavior = Behaviors::stopped_with_post_stop::<u32, _>(move || {
    *called_clone.lock() = true;
  });

  let result = behavior.handle_signal(&mut typed_ctx, &BehaviorSignal::from(PreRestart)).expect("should handle");
  assert!(
    matches!(result.directive(), crate::core::typed::behavior::BehaviorDirective::Same),
    "non-PostStop signals should return Same"
  );
  assert!(!*called.lock(), "callback should not be invoked for PreRestart signal");
}

/// Ensures that tracing's global callsite interest cache does not permanently
/// disable callsites before any subscriber is set.
fn ensure_tracing_interest_cache_permissive() {
  static INIT: Once = Once::new();
  INIT.call_once(|| {
    struct PermissiveGlobalSubscriber;

    impl Subscriber for PermissiveGlobalSubscriber {
      fn register_callsite(&self, _: &'static Metadata<'static>) -> Interest {
        Interest::sometimes()
      }

      fn enabled(&self, _: &Metadata<'_>) -> bool {
        false
      }

      fn new_span(&self, _: &Attributes<'_>) -> Id {
        Id::from_u64(1)
      }

      fn record(&self, _: &Id, _: &Record<'_>) {}

      fn record_follows_from(&self, _: &Id, _: &Id) {}

      fn event(&self, _: &Event<'_>) {}

      fn enter(&self, _: &Id) {}

      fn exit(&self, _: &Id) {}
    }

    if let Err(error) = tracing::subscriber::set_global_default(PermissiveGlobalSubscriber) {
      panic!("failed to set permissive global tracing subscriber: {error}");
    }
  });
}

#[derive(Clone, Debug)]
struct CapturedEvent {
  level:       Level,
  logger_name: Option<String>,
}

#[derive(Clone, Default)]
struct RecordingTracingSubscriber {
  events: Arc<Mutex<Vec<CapturedEvent>>>,
}

impl RecordingTracingSubscriber {
  fn events(&self) -> Vec<CapturedEvent> {
    self.events.lock().expect("lock").clone()
  }
}

impl Subscriber for RecordingTracingSubscriber {
  fn enabled(&self, _metadata: &Metadata<'_>) -> bool {
    true
  }

  fn new_span(&self, _: &Attributes<'_>) -> Id {
    Id::from_u64(1)
  }

  fn record(&self, _: &Id, _: &Record<'_>) {}

  fn record_follows_from(&self, _: &Id, _: &Id) {}

  fn event(&self, event: &Event<'_>) {
    let mut visitor = EventVisitor::default();
    event.record(&mut visitor);
    self
      .events
      .lock()
      .expect("lock")
      .push(CapturedEvent { level: *event.metadata().level(), logger_name: visitor.logger_name });
  }

  fn enter(&self, _: &Id) {}

  fn exit(&self, _: &Id) {}
}

#[derive(Default)]
struct EventVisitor {
  logger_name: Option<String>,
}

impl Visit for EventVisitor {
  fn record_str(&mut self, field: &Field, value: &str) {
    if field.name() == "logger_name" {
      self.logger_name = Some(value.to_owned());
    }
  }

  fn record_debug(&mut self, _field: &Field, _value: &dyn core::fmt::Debug) {}
}

#[derive(Clone, Debug)]
struct CapturedSpan {
  name: String,
}

#[derive(Clone, Default)]
struct SpanRecordingSubscriber {
  spans: Arc<Mutex<Vec<CapturedSpan>>>,
}

impl SpanRecordingSubscriber {
  fn spans(&self) -> Vec<CapturedSpan> {
    self.spans.lock().expect("lock").clone()
  }
}

impl Subscriber for SpanRecordingSubscriber {
  fn register_callsite(&self, _metadata: &'static Metadata<'static>) -> Interest {
    Interest::sometimes()
  }

  fn enabled(&self, _metadata: &Metadata<'_>) -> bool {
    true
  }

  fn max_level_hint(&self) -> Option<LevelFilter> {
    Some(LevelFilter::TRACE)
  }

  fn new_span(&self, attrs: &Attributes<'_>) -> Id {
    let name = attrs.metadata().name().into();
    let mut spans = self.spans.lock().expect("lock");
    spans.push(CapturedSpan { name });
    Id::from_u64(spans.len() as u64)
  }

  fn record(&self, _: &Id, _: &Record<'_>) {}

  fn record_follows_from(&self, _: &Id, _: &Id) {}

  fn event(&self, _: &Event<'_>) {}

  fn enter(&self, _: &Id) {}

  fn exit(&self, _: &Id) {}
}

#[test]
fn log_messages_delegates_to_inner_behavior() {
  let inner_received = ArcShared::new(SpinSyncMutex::new(Vec::<u32>::new()));
  let inner_received_clone = inner_received.clone();

  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  let mut behavior = Behaviors::log_messages(Behaviors::receive_message(move |_ctx, msg: &u32| {
    inner_received_clone.lock().push(*msg);
    Ok(Behaviors::same())
  }));
  let mut inner = behavior.handle_start(&mut typed_ctx).expect("started");
  inner.handle_message(&mut typed_ctx, &77_u32).expect("message");

  assert_eq!(inner_received.lock().as_slice(), &[77]);
}

#[test]
fn log_messages_with_opts_records_level_and_logger_name() {
  ensure_tracing_interest_cache_permissive();
  let collector = RecordingTracingSubscriber::default();
  let shared = collector.clone();

  with_default(shared, || {
    let options = LogOptions::new().with_level(LogLevel::Info).with_logger_name("typed.behaviors");
    let mut behavior =
      Behaviors::log_messages_with_opts(options, Behaviors::receive_message(|_ctx, _msg: &u32| Ok(Behaviors::same())));

    let system = ActorSystem::new_empty();
    let pid = system.allocate_pid();
    let mut context = ActorContext::new(&system, pid);
    let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

    let mut inner = behavior.handle_start(&mut typed_ctx).expect("started");
    inner.handle_message(&mut typed_ctx, &91_u32).expect("message");
  });

  let events = collector.events();
  assert_eq!(events.len(), 1);
  assert_eq!(events[0].level, Level::INFO);
  assert_eq!(events[0].logger_name.as_deref(), Some("typed.behaviors"));
}

#[test]
fn with_static_mdc_creates_span_on_message() {
  ensure_tracing_interest_cache_permissive();
  let collector = SpanRecordingSubscriber::default();
  let shared = collector.clone();

  with_default(shared, || {
    let mut mdc = BTreeMap::new();
    mdc.insert("service".into(), "my-actor".into());

    let mut behavior =
      Behaviors::with_static_mdc(mdc, Behaviors::receive_message(|_ctx, _msg: &u32| Ok(Behaviors::same())));

    let system = ActorSystem::new_empty();
    let pid = system.allocate_pid();
    let mut context = ActorContext::new(&system, pid);
    let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

    let mut inner = behavior.handle_start(&mut typed_ctx).expect("started");
    inner.handle_message(&mut typed_ctx, &42_u32).expect("message");

    let spans = collector.spans();
    assert!(!spans.is_empty());
    assert!(spans.iter().any(|span| span.name == "actor_mdc"));
  });
}

#[test]
fn with_static_mdc_creates_span_on_signal() {
  ensure_tracing_interest_cache_permissive();
  let collector = SpanRecordingSubscriber::default();
  let shared = collector.clone();

  with_default(shared, || {
    let mut mdc = BTreeMap::new();
    mdc.insert("service".into(), "signal-actor".into());

    let mut behavior =
      Behaviors::with_static_mdc(mdc, Behaviors::receive_message(|_ctx, _msg: &u32| Ok(Behaviors::same())));

    let system = ActorSystem::new_empty();
    let pid = system.allocate_pid();
    let mut context = ActorContext::new(&system, pid);
    let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

    let mut inner = behavior.handle_start(&mut typed_ctx).expect("started");
    inner.handle_signal(&mut typed_ctx, &BehaviorSignal::PostStop).expect("handle_signal PostStop failed");

    let spans = collector.spans();
    assert!(!spans.is_empty());
    assert!(spans.iter().any(|span| span.name == "actor_mdc"));
  });
}

#[test]
fn with_mdc_delegates_to_inner_behavior() {
  ensure_tracing_interest_cache_permissive();
  let inner_received = ArcShared::new(SpinSyncMutex::new(Vec::<u32>::new()));
  let inner_received_clone = inner_received.clone();

  let mut static_mdc = BTreeMap::new();
  static_mdc.insert("service".into(), "test-actor".into());

  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  let mut behavior = Behaviors::with_mdc(
    static_mdc,
    |msg: &u32| {
      let mut mdc = BTreeMap::new();
      mdc.insert("msg_value".into(), alloc::format!("{msg}"));
      mdc
    },
    Behaviors::receive_message(move |_ctx, msg: &u32| {
      inner_received_clone.lock().push(*msg);
      Ok(Behaviors::same())
    }),
  );

  let mut inner = behavior.handle_start(&mut typed_ctx).expect("started");
  inner.handle_message(&mut typed_ctx, &66_u32).expect("message");

  assert_eq!(inner_received.lock().as_slice(), &[66]);
}

#[test]
fn with_message_mdc_creates_actor_mdc_span() {
  ensure_tracing_interest_cache_permissive();
  let collector = SpanRecordingSubscriber::default();
  let shared = collector.clone();

  with_default(shared, || {
    let mut behavior = Behaviors::with_message_mdc(
      |msg: &u32| {
        let mut mdc = BTreeMap::new();
        mdc.insert("msg_id".into(), alloc::format!("{msg}"));
        mdc
      },
      Behaviors::receive_message(|_ctx, _msg: &u32| Ok(Behaviors::same())),
    );

    let system = ActorSystem::new_empty();
    let pid = system.allocate_pid();
    let mut context = ActorContext::new(&system, pid);
    let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

    let mut inner = behavior.handle_start(&mut typed_ctx).expect("started");
    inner.handle_message(&mut typed_ctx, &42_u32).expect("message");

    let spans = collector.spans();
    assert!(!spans.is_empty());
    assert!(spans.iter().any(|span| span.name == "actor_mdc"));
  });
}
