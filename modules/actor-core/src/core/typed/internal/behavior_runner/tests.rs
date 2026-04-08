use alloc::{string::String, sync::Arc};
use core::sync::atomic::{AtomicBool, Ordering};

use fraktor_utils_core_rs::core::sync::{ArcShared, NoStdMutex};

use super::BehaviorRunner;
use crate::core::{
  kernel::{
    actor::{ActorContext, error::ActorError},
    event::stream::{EventStreamEvent, EventStreamSubscriber, subscriber_handle},
    system::ActorSystem,
  },
  typed::{
    actor::{TypedActor, TypedActorContext},
    behavior::Behavior,
    dsl::Behaviors,
    internal::behavior_runner::{AdapterFailureEvent, Pid},
    message_adapter::{AdapterError, MessageAdapterRegistry},
    message_and_signals::{BehaviorSignal, DeathPactError},
  },
};

struct ProbeMessage;

struct RecordingUnhandledSubscriber {
  events: ArcShared<NoStdMutex<Vec<EventStreamEvent>>>,
}

impl RecordingUnhandledSubscriber {
  fn new(events: ArcShared<NoStdMutex<Vec<EventStreamEvent>>>) -> Self {
    Self { events }
  }
}

impl EventStreamSubscriber for RecordingUnhandledSubscriber {
  fn on_event(&mut self, event: &EventStreamEvent) {
    if matches!(event, EventStreamEvent::UnhandledMessage(_)) {
      self.events.lock().push(event.clone());
    }
  }
}

struct RecordingAdapterFailureSubscriber {
  events: ArcShared<NoStdMutex<Vec<EventStreamEvent>>>,
}

impl RecordingAdapterFailureSubscriber {
  fn new(events: ArcShared<NoStdMutex<Vec<EventStreamEvent>>>) -> Self {
    Self { events }
  }
}

impl EventStreamSubscriber for RecordingAdapterFailureSubscriber {
  fn on_event(&mut self, event: &EventStreamEvent) {
    if matches!(event, EventStreamEvent::AdapterFailure(_)) {
      self.events.lock().push(event.clone());
    }
  }
}

fn build_context() -> (ActorContext<'static>, MessageAdapterRegistry<ProbeMessage>) {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let ctx = ActorContext::new(&system, pid);
  (ctx, MessageAdapterRegistry::new())
}

fn build_context_with_pids(count: usize) -> (ActorSystem, Vec<Pid>) {
  let system = ActorSystem::new_empty();
  let pids: Vec<_> = (0..count).map(|_| system.allocate_pid()).collect();
  (system, pids)
}

fn signal_probe_behavior(
  target_signal: fn(&BehaviorSignal) -> bool,
  witness: Arc<AtomicBool>,
) -> Behavior<ProbeMessage> {
  Behaviors::receive_signal(move |_, signal| {
    if target_signal(signal) {
      witness.store(true, Ordering::SeqCst);
    }
    Ok(Behaviors::same())
  })
}

#[test]
fn behavior_runner_escalates_without_signal_handler() {
  let behavior = Behaviors::receive_message(|_, _msg: &ProbeMessage| Ok(Behaviors::same()));
  let mut runner = BehaviorRunner::new(behavior);
  let (mut ctx, mut registry) = build_context();
  let mut typed_ctx = TypedActorContext::from_untyped(&mut ctx, Some(&mut registry));
  let result = runner.on_adapter_failure(&mut typed_ctx, AdapterError::Custom(String::from("boom")));
  assert!(result.is_err());
}

#[test]
fn behavior_runner_allows_handled_adapter_failure() {
  let handled = Arc::new(AtomicBool::new(false));
  let behavior = signal_probe_behavior(|s| matches!(s, BehaviorSignal::MessageAdaptionFailure(_)), handled.clone());
  let mut runner = BehaviorRunner::new(behavior);
  let (mut ctx, mut registry) = build_context();
  let mut typed_ctx = TypedActorContext::from_untyped(&mut ctx, Some(&mut registry));
  let result = runner.on_adapter_failure(&mut typed_ctx, AdapterError::Custom(String::from("oops")));
  assert!(result.is_ok());
  assert!(handled.load(Ordering::SeqCst));
}

#[test]
fn behavior_runner_publishes_adapter_failure_event() {
  let system = ActorSystem::new_empty();
  let events = ArcShared::new(NoStdMutex::new(Vec::new()));
  let subscriber = subscriber_handle(RecordingAdapterFailureSubscriber::new(events.clone()));
  let _subscription = system.subscribe_event_stream(&subscriber);

  let pid = system.allocate_pid();
  let mut ctx = ActorContext::new(&system, pid);
  let mut registry = MessageAdapterRegistry::<ProbeMessage>::new();
  let mut typed_ctx = TypedActorContext::from_untyped(&mut ctx, Some(&mut registry));
  let behavior = Behaviors::receive_message(|_, _msg: &ProbeMessage| Ok(Behaviors::same()));
  let mut runner = BehaviorRunner::new(behavior);

  let result = runner.on_adapter_failure(&mut typed_ctx, AdapterError::Custom(String::from("boom")));

  assert!(result.is_err());
  let recorded = events.lock();
  assert_eq!(recorded.len(), 1);
  match &recorded[0] {
    | EventStreamEvent::AdapterFailure(event) => match event {
      | AdapterFailureEvent::Custom { pid: event_pid, detail } => {
        assert_eq!(*event_pid, pid);
        assert_eq!(detail, "boom");
      },
      | _ => panic!("Expected custom adapter failure event"),
    },
    | _ => panic!("Expected AdapterFailure event"),
  }
}

#[test]
fn behavior_runner_publishes_unhandled_message_event_for_unhandled_behavior() {
  let system = ActorSystem::new_empty();
  let events = ArcShared::new(NoStdMutex::new(Vec::new()));
  let subscriber = subscriber_handle(RecordingUnhandledSubscriber::new(events.clone()));
  let _subscription = system.subscribe_event_stream(&subscriber);

  let pid = system.allocate_pid();
  let mut ctx = ActorContext::new(&system, pid);
  let mut registry = MessageAdapterRegistry::<ProbeMessage>::new();
  let mut typed_ctx = TypedActorContext::from_untyped(&mut ctx, Some(&mut registry));
  let behavior = Behaviors::receive_message(|_, _msg: &ProbeMessage| Ok(Behaviors::unhandled()));
  let mut runner = BehaviorRunner::new(behavior);

  let result = runner.receive(&mut typed_ctx, &ProbeMessage);

  assert!(result.is_ok());
  let recorded = events.lock();
  assert_eq!(recorded.len(), 1);
  match &recorded[0] {
    | EventStreamEvent::UnhandledMessage(event) => {
      assert_eq!(event.actor(), pid);
      assert_eq!(event.message(), core::any::type_name::<ProbeMessage>());
      assert!(event.timestamp() <= system.state().monotonic_now());
    },
    | _ => panic!("Expected UnhandledMessage event"),
  }
}

#[test]
fn behavior_runner_publishes_unhandled_message_event_for_empty_behavior() {
  let system = ActorSystem::new_empty();
  let events = ArcShared::new(NoStdMutex::new(Vec::new()));
  let subscriber = subscriber_handle(RecordingUnhandledSubscriber::new(events.clone()));
  let _subscription = system.subscribe_event_stream(&subscriber);

  let pid = system.allocate_pid();
  let mut ctx = ActorContext::new(&system, pid);
  let mut registry = MessageAdapterRegistry::<ProbeMessage>::new();
  let mut typed_ctx = TypedActorContext::from_untyped(&mut ctx, Some(&mut registry));
  let mut runner = BehaviorRunner::new(Behaviors::empty());

  let result = runner.receive(&mut typed_ctx, &ProbeMessage);

  assert!(result.is_ok());
  let recorded = events.lock();
  assert_eq!(recorded.len(), 1);
  match &recorded[0] {
    | EventStreamEvent::UnhandledMessage(event) => {
      assert_eq!(event.actor(), pid);
      assert_eq!(event.message(), core::any::type_name::<ProbeMessage>());
      assert!(event.timestamp() <= system.state().monotonic_now());
    },
    | _ => panic!("Expected UnhandledMessage event"),
  }
}

#[test]
fn behavior_runner_dispatches_pre_restart_signal() {
  let received = Arc::new(AtomicBool::new(false));
  let behavior = signal_probe_behavior(|s| matches!(s, BehaviorSignal::PreRestart), received.clone());
  let mut runner = BehaviorRunner::new(behavior);
  let (mut ctx, mut registry) = build_context();
  let mut typed_ctx = TypedActorContext::from_untyped(&mut ctx, Some(&mut registry));
  let result = runner.pre_restart(&mut typed_ctx);
  assert!(result.is_ok());
  assert!(received.load(Ordering::SeqCst));
}

#[test]
fn behavior_runner_dispatches_post_stop_signal() {
  let received = Arc::new(AtomicBool::new(false));
  let behavior = signal_probe_behavior(|s| matches!(s, BehaviorSignal::PostStop), received.clone());
  let mut runner = BehaviorRunner::new(behavior);
  let (mut ctx, mut registry) = build_context();
  let mut typed_ctx = TypedActorContext::from_untyped(&mut ctx, Some(&mut registry));
  let result = runner.post_stop(&mut typed_ctx);
  assert!(result.is_ok());
  assert!(received.load(Ordering::SeqCst));
}

#[test]
fn behavior_runner_pre_start_uses_internal_setup_without_public_started_signal() {
  let setup_count = Arc::new(AtomicBool::new(false));
  let signal_received = Arc::new(AtomicBool::new(false));
  let setup_count_for_factory = setup_count.clone();
  let signal_received_for_signal = signal_received.clone();

  let behavior = Behaviors::setup(move |_ctx| {
    setup_count_for_factory.store(true, Ordering::SeqCst);
    let signal_received = signal_received_for_signal.clone();
    Behaviors::receive_signal(move |_ctx, signal| {
      if matches!(signal, BehaviorSignal::PostStop) {
        signal_received.store(true, Ordering::SeqCst);
      }
      Ok(Behaviors::same())
    })
  });
  let mut runner = BehaviorRunner::new(behavior);
  let (mut ctx, mut registry) = build_context();
  let mut typed_ctx = TypedActorContext::from_untyped(&mut ctx, Some(&mut registry));

  runner.pre_start(&mut typed_ctx).expect("pre_start");

  assert!(setup_count.load(Ordering::SeqCst), "setup factory should run during pre_start");
  assert!(!signal_received.load(Ordering::SeqCst), "pre_start must not dispatch any public signal");

  runner.post_stop(&mut typed_ctx).expect("post_stop");

  assert!(signal_received.load(Ordering::SeqCst), "public signal handling should remain available after setup");
}

#[test]
fn behavior_runner_resolves_nested_setup_returned_from_message_transition() {
  let setup_ran = Arc::new(AtomicBool::new(false));
  let handled = Arc::new(AtomicBool::new(false));
  let setup_ran_for_message = setup_ran.clone();
  let handled_for_nested = handled.clone();

  let behavior = Behaviors::receive_message(move |_ctx, _msg: &ProbeMessage| {
    let setup_ran = setup_ran_for_message.clone();
    let handled = handled_for_nested.clone();
    Ok(Behaviors::setup(move |_ctx| {
      setup_ran.store(true, Ordering::SeqCst);
      let handled = handled.clone();
      Behaviors::receive_message(move |_ctx, _msg: &ProbeMessage| {
        handled.store(true, Ordering::SeqCst);
        Ok(Behaviors::same())
      })
    }))
  });

  let mut runner = BehaviorRunner::new(behavior);
  let (mut ctx, mut registry) = build_context();
  let mut typed_ctx = TypedActorContext::from_untyped(&mut ctx, Some(&mut registry));

  runner.pre_start(&mut typed_ctx).expect("pre_start");
  runner.receive(&mut typed_ctx, &ProbeMessage).expect("first message");
  assert!(setup_ran.load(Ordering::SeqCst), "nested setup should run during transition");

  runner.receive(&mut typed_ctx, &ProbeMessage).expect("second message");
  assert!(handled.load(Ordering::SeqCst), "behavior returned by nested setup should become current");
}

#[test]
fn behavior_runner_pre_start_does_not_mark_stopping_when_stop_self_fails() {
  let mut runner = BehaviorRunner::new(Behaviors::stopped::<ProbeMessage>());
  let (mut ctx, mut registry) = build_context();
  let mut typed_ctx = TypedActorContext::from_untyped(&mut ctx, Some(&mut registry));

  let first = runner.pre_start(&mut typed_ctx);
  assert!(first.is_err());
  assert!(!runner.stopping, "stop_self 失敗時に stopping を保持してはならない");

  let second = runner.pre_start(&mut typed_ctx);
  assert!(second.is_err());
  assert!(!runner.stopping, "再試行可能性を維持するため stopping は false のままであるべき");
}

#[test]
fn behavior_runner_dispatches_child_failed_signal() {
  let received = Arc::new(AtomicBool::new(false));
  let behavior = signal_probe_behavior(|s| matches!(s, BehaviorSignal::ChildFailed { .. }), received.clone());
  let mut runner = BehaviorRunner::new(behavior);
  let (system, pids) = build_context_with_pids(2);
  let mut ctx = ActorContext::new(&system, pids[0]);
  let mut registry = MessageAdapterRegistry::<ProbeMessage>::new();
  let mut typed_ctx = TypedActorContext::from_untyped(&mut ctx, Some(&mut registry));
  let error = ActorError::recoverable("child boom");
  let result = runner.on_child_failed(&mut typed_ctx, pids[1], &error);
  assert!(result.is_ok());
  assert!(received.load(Ordering::SeqCst));
}

#[test]
fn behavior_runner_death_pact_errors_without_signal_handler() {
  let behavior = Behaviors::receive_message(|_, _msg: &ProbeMessage| Ok(Behaviors::same()));
  let mut runner = BehaviorRunner::new(behavior);
  let (system, pids) = build_context_with_pids(2);
  let mut ctx = ActorContext::new(&system, pids[0]);
  let mut registry = MessageAdapterRegistry::<ProbeMessage>::new();
  let mut typed_ctx = TypedActorContext::from_untyped(&mut ctx, Some(&mut registry));
  let result = runner.on_terminated(&mut typed_ctx, pids[1]);
  let error = result.unwrap_err();
  assert!(error.is_source_type::<DeathPactError>(), "error should be typed as DeathPactError");
  assert!(error.reason().as_str().contains("death pact"), "message should describe death pact");
}

#[test]
fn behavior_runner_death_pact_succeeds_with_signal_handler() {
  let received = Arc::new(AtomicBool::new(false));
  let behavior = signal_probe_behavior(|s| matches!(s, BehaviorSignal::Terminated(_)), received.clone());
  let mut runner = BehaviorRunner::new(behavior);
  let (system, pids) = build_context_with_pids(2);
  let mut ctx = ActorContext::new(&system, pids[0]);
  let mut registry = MessageAdapterRegistry::<ProbeMessage>::new();
  let mut typed_ctx = TypedActorContext::from_untyped(&mut ctx, Some(&mut registry));
  let result = runner.on_terminated(&mut typed_ctx, pids[1]);
  assert!(result.is_ok());
  assert!(received.load(Ordering::SeqCst));
}

/// Regression test: when a signal handler returns `Behaviors::unhandled()`,
/// `DeathPactError` must be emitted.
#[test]
fn behavior_runner_death_pact_errors_when_handler_returns_unhandled() {
  let behavior = Behaviors::receive_signal(|_, _signal| Ok(Behaviors::unhandled()));
  let mut runner = BehaviorRunner::new(behavior);
  let (system, pids) = build_context_with_pids(2);
  let mut ctx = ActorContext::new(&system, pids[0]);
  let mut registry = MessageAdapterRegistry::<ProbeMessage>::new();
  let mut typed_ctx = TypedActorContext::from_untyped(&mut ctx, Some(&mut registry));
  let result = runner.on_terminated(&mut typed_ctx, pids[1]);
  let error = result.unwrap_err();
  assert!(error.is_source_type::<DeathPactError>(), "handler が Unhandled を返した場合も DeathPactError になるべき");
  assert!(error.reason().as_str().contains("death pact"), "メッセージに death pact が含まれるべき");
}

/// Regression: `stopped_with_post_stop` returned from a message handler must
/// still invoke the callback when `post_stop` dispatches `BehaviorSignal::PostStop`.
///
/// Previously `apply_transition` unconditionally replaced `self.current` with
/// a plain `Behavior::stopped()` (no signal handler), silently discarding the
/// callback before `post_stop` could run it.
#[test]
fn behavior_runner_post_stop_callback_runs_when_stopped_returned_from_message_handler() {
  let called = Arc::new(AtomicBool::new(false));
  let called_for_post_stop = called.clone();

  let behavior = Behaviors::receive_message(move |_, _msg: &ProbeMessage| {
    let cb_ref = called_for_post_stop.clone();
    Ok(Behaviors::stopped_with_post_stop(move || {
      cb_ref.store(true, Ordering::SeqCst);
    }))
  });

  let (mut ctx, mut registry) = build_context();
  let mut typed_ctx = TypedActorContext::from_untyped(&mut ctx, Some(&mut registry));
  let mut runner = BehaviorRunner::new(behavior);

  // stopped_with_post_stop への遷移を模倣するメッセージ配信。
  // 最小テスト環境（アクターセル未登録）では stop_self() が失敗する場合があるが、
  // post_stop がハンドラを呼び出せるようシグナルハンドラは必ず保持されなければならない。
  if let Err(_stop_err) = runner.receive(&mut typed_ctx, &ProbeMessage) {
    // stop_self の失敗のみ許容。シグナルハンドラは保持されているはず。
  }

  // post_stop ライフサイクルコールバックを模倣する。
  runner.post_stop(&mut typed_ctx).expect("post_stop");

  assert!(
    called.load(Ordering::SeqCst),
    "post_stop callback must run after stopped_with_post_stop is returned from message handler"
  );
}

#[test]
fn behavior_runner_post_stop_from_empty_does_not_publish_unhandled_message() {
  let system = ActorSystem::new_empty();
  let events = ArcShared::new(NoStdMutex::new(Vec::new()));
  let subscriber = subscriber_handle(RecordingUnhandledSubscriber::new(events.clone()));
  let _subscription = system.subscribe_event_stream(&subscriber);

  let pid = system.allocate_pid();
  let mut ctx = ActorContext::new(&system, pid);
  let mut registry = MessageAdapterRegistry::<ProbeMessage>::new();
  let mut typed_ctx = TypedActorContext::from_untyped(&mut ctx, Some(&mut registry));
  let mut runner = BehaviorRunner::new(Behaviors::empty());

  let result = runner.post_stop(&mut typed_ctx);

  assert!(result.is_ok());
  assert!(events.lock().is_empty());
}
