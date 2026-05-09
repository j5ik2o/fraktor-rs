use alloc::{
  boxed::Box,
  string::{String, ToString},
  sync::Arc,
  vec::Vec,
};
use core::{
  convert::Infallible,
  hint::spin_loop,
  num::NonZeroUsize,
  sync::atomic::{AtomicUsize, Ordering},
  time::Duration,
};

use fraktor_actor_adaptor_std_rs::std::tick_driver::TestTickDriver;
use fraktor_actor_core_kernel_rs::{
  actor::{
    actor_ref::dead_letter::{DeadLetterEntry, DeadLetterReason},
    error::ActorError,
    messaging::AnyMessage,
    setup::ActorSystemConfig,
    supervision::{
      BackoffSupervisorStrategy, RestartLimit, SupervisorDirective, SupervisorStrategy, SupervisorStrategyConfig,
      SupervisorStrategyKind,
    },
  },
  system::SpinBlocker,
};
use fraktor_utils_core_rs::core::sync::SpinSyncMutex;

use crate::{
  Behavior, DispatcherSelector, ExtensibleBehavior, TypedActorRef,
  actor::{TypedActor, TypedActorContext},
  behavior_interceptor::BehaviorInterceptor,
  dsl::{Behaviors, StashBuffer, TypedAskError},
  message_adapter::{AdapterEnvelope, AdapterError, AdapterPayload},
  message_and_signals::{
    BehaviorSignal, ChildFailed, MessageAdaptionFailure, PostStop, PreRestart, Signal, Terminated,
  },
  props::TypedProps,
  system::TypedActorSystem,
};

#[derive(Clone)]
enum CounterMessage {
  Increment(i32),
  Get { reply_to: TypedActorRef<i32> },
}

#[derive(Clone)]
enum IgnoreCommand {
  Add(u32),
  Reject,
  Read { reply_to: TypedActorRef<u32> },
}

#[derive(Clone)]
enum StashCommand {
  Buffer(u32),
  Open,
  Read { reply_to: TypedActorRef<u32> },
}

#[derive(Clone)]
enum StashOrderCommand {
  Buffer(String),
  Marker(String),
  Open,
  Read { reply_to: TypedActorRef<Vec<String>> },
}

#[derive(Clone)]
enum AdapterCounterCommand {
  Set(i32),
  Read { reply_to: TypedActorRef<i32> },
}

struct CounterActor {
  total: i32,
}

struct RootExtensibleBehaviorProbe;

impl ExtensibleBehavior<LifecycleCommand> for RootExtensibleBehaviorProbe {
  fn receive(
    &mut self,
    _ctx: &mut TypedActorContext<'_, LifecycleCommand>,
    _message: &LifecycleCommand,
  ) -> Result<Behavior<LifecycleCommand>, ActorError> {
    Ok(Behaviors::same())
  }
}

impl CounterActor {
  const fn new() -> Self {
    Self { total: 0 }
  }
}

impl TypedActor<CounterMessage> for CounterActor {
  fn receive(
    &mut self,
    _ctx: &mut TypedActorContext<'_, CounterMessage>,
    message: &CounterMessage,
  ) -> Result<(), ActorError> {
    match message {
      | CounterMessage::Increment(delta) => {
        self.total += delta;
        Ok(())
      },
      | CounterMessage::Get { reply_to } => {
        let mut reply_to = reply_to.clone();
        reply_to.tell(self.total);
        Ok(())
      },
    }
  }
}

#[test]
fn typed_actor_system_handles_basic_flow() {
  let props = TypedProps::<CounterMessage>::new(CounterActor::new);
  let system =
    TypedActorSystem::<CounterMessage>::create_from_props(&props, ActorSystemConfig::new(TestTickDriver::default()))
      .expect("system");
  let mut counter = system.user_guardian_ref();

  counter.tell(CounterMessage::Increment(2));
  counter.tell(CounterMessage::Increment(5));

  let response = counter.ask::<i32, _>(|reply_to| CounterMessage::Get { reply_to });
  let mut future = response.future().clone();
  wait_until(|| future.is_ready());
  let payload = future.try_take().expect("reply available").expect("typed payload");

  assert_eq!(payload, 7);

  system.terminate().expect("terminate");
}

#[test]
fn typed_props_with_blocking_dispatcher_selector_should_spawn() {
  let props = TypedProps::<CounterMessage>::from_behavior_factory(|| behavior_counter(0))
    .with_dispatcher_selector(DispatcherSelector::Blocking);
  let system =
    TypedActorSystem::<CounterMessage>::create_from_props(&props, ActorSystemConfig::new(TestTickDriver::default()))
      .expect("system");

  system.terminate().expect("terminate");
}

#[test]
fn typed_props_with_same_as_parent_dispatcher_selector_marks_parent_inheritance() {
  let props = TypedProps::<CounterMessage>::from_behavior_factory(|| behavior_counter(0))
    .with_dispatcher_selector(DispatcherSelector::SameAsParent);

  assert!(props.to_untyped().dispatcher_same_as_parent());
}

#[test]
fn typed_props_empty_supports_immutable_builder_chain() {
  // Given: factory を持たない empty typed props がある
  let props = TypedProps::<CounterMessage>::empty();
  let capacity = NonZeroUsize::new(8).expect("capacity");

  // When: dispatcher / mailbox / tags を immutable builder で合成する
  let configured = props
    .clone()
    .with_dispatcher_selector(DispatcherSelector::SameAsParent)
    .with_mailbox_bounded(capacity)
    .with_tags(["phase2-empty", "typed-props"]);

  // Then: 元の props は空のままで、派生 props にだけ設定が積まれる
  assert!(!props.to_untyped().dispatcher_same_as_parent());
  assert!(props.to_untyped().tags().is_empty());
  assert_eq!(
    props.to_untyped().mailbox_policy().capacity(),
    fraktor_actor_core_kernel_rs::dispatch::mailbox::MailboxCapacity::Unbounded
  );

  assert!(configured.to_untyped().dispatcher_same_as_parent());
  assert_eq!(
    configured.to_untyped().mailbox_policy().capacity(),
    fraktor_actor_core_kernel_rs::dispatch::mailbox::MailboxCapacity::Bounded { capacity }
  );
  assert!(configured.to_untyped().tags().contains("phase2-empty"));
  assert!(configured.to_untyped().tags().contains("typed-props"));
}

#[test]
fn typed_props_with_mailbox_unbounded_overrides_bounded_selector() {
  let capacity = NonZeroUsize::new(8).expect("capacity");
  let props = TypedProps::<CounterMessage>::empty().with_mailbox_bounded(capacity).with_mailbox_unbounded();

  assert_eq!(
    props.to_untyped().mailbox_policy().capacity(),
    fraktor_actor_core_kernel_rs::dispatch::mailbox::MailboxCapacity::Unbounded
  );
}

#[test]
fn typed_behaviors_handle_recursive_state() {
  let props = TypedProps::<CounterMessage>::from_behavior_factory(|| behavior_counter(0));
  let system =
    TypedActorSystem::<CounterMessage>::create_from_props(&props, ActorSystemConfig::new(TestTickDriver::default()))
      .expect("system");
  let mut counter = system.user_guardian_ref();

  counter.tell(CounterMessage::Increment(3));
  counter.tell(CounterMessage::Increment(5));

  let response = counter.ask::<i32, _>(|reply_to| CounterMessage::Get { reply_to });
  let mut future = response.future().clone();
  wait_until(|| future.is_ready());
  let payload = future.try_take().expect("reply available").expect("typed payload");

  assert_eq!(payload, 8);

  system.terminate().expect("terminate");
}

#[test]
fn typed_behaviors_ignore_keeps_current_state() {
  let props = TypedProps::<IgnoreCommand>::from_behavior_factory(|| ignore_gate(0));
  let system =
    TypedActorSystem::<IgnoreCommand>::create_from_props(&props, ActorSystemConfig::new(TestTickDriver::default()))
      .expect("system");
  let mut gate = system.user_guardian_ref();

  gate.tell(IgnoreCommand::Add(1));
  gate.tell(IgnoreCommand::Reject);
  gate.tell(IgnoreCommand::Add(5));

  let response = gate.ask::<u32, _>(|reply_to| IgnoreCommand::Read { reply_to });
  let mut future = response.future().clone();
  wait_until(|| future.is_ready());
  let payload = future.try_take().expect("reply available").expect("typed payload");

  assert_eq!(payload, 6);

  system.terminate().expect("terminate");
}

#[test]
fn typed_behaviors_stash_buffered_messages_across_transition() {
  let props = TypedProps::<StashCommand>::from_behavior_factory(|| stash_behavior(0)).with_stash_mailbox();
  let system =
    TypedActorSystem::<StashCommand>::create_from_props(&props, ActorSystemConfig::new(TestTickDriver::default()))
      .expect("system");
  let mut actor = system.user_guardian_ref();

  actor.tell(StashCommand::Buffer(4));
  actor.tell(StashCommand::Buffer(3));
  actor.tell(StashCommand::Open);

  wait_until(|| read_stash_total(&mut actor) == 7);
  assert_eq!(read_stash_total(&mut actor), 7);

  system.terminate().expect("terminate");
}

#[test]
fn typed_behaviors_with_stash_limits_capacity() {
  let overflow_count = Arc::new(AtomicUsize::new(0));
  let overflow_probe = Arc::clone(&overflow_count);
  let props = TypedProps::<StashCommand>::from_behavior_factory(move || {
    stash_behavior_with_capacity_limit(0, Arc::clone(&overflow_probe))
  })
  .with_stash_mailbox();
  let system =
    TypedActorSystem::<StashCommand>::create_from_props(&props, ActorSystemConfig::new(TestTickDriver::default()))
      .expect("system");
  let mut actor = system.user_guardian_ref();

  actor.tell(StashCommand::Buffer(4));
  actor.tell(StashCommand::Buffer(3));
  actor.tell(StashCommand::Open);

  wait_until(|| read_stash_total(&mut actor) == 4);
  assert_eq!(read_stash_total(&mut actor), 4);
  assert_eq!(overflow_count.load(Ordering::SeqCst), 1);

  system.terminate().expect("terminate");
}

#[test]
fn typed_behaviors_with_stash_keeps_adapter_payload_after_unstash() {
  let adapter_slot: Arc<SpinSyncMutex<Option<TypedActorRef<i32>>>> = Arc::new(SpinSyncMutex::new(None));
  let props = TypedProps::<StashCommand>::from_behavior_factory({
    let slot = adapter_slot.clone();
    move || adapter_stash_behavior(0, &slot)
  })
  .with_stash_mailbox();
  let system =
    TypedActorSystem::<StashCommand>::create_from_props(&props, ActorSystemConfig::new(TestTickDriver::default()))
      .expect("system");
  let mut actor = system.user_guardian_ref();

  assert!(wait_for(|| adapter_slot.lock().is_some()), "adapter never registered");
  let mut adapter = adapter_slot.lock().clone().expect("adapter available");

  adapter.tell(4);
  adapter.tell(3);
  actor.tell(StashCommand::Open);

  wait_until(|| read_stash_total(&mut actor) == 7);
  assert_eq!(read_stash_total(&mut actor), 7);

  system.terminate().expect("terminate");
}

#[test]
fn typed_behaviors_unstash_replays_before_already_queued_messages() {
  let props =
    TypedProps::<StashOrderCommand>::from_behavior_factory(|| stash_order_behavior(Vec::new())).with_stash_mailbox();
  let system =
    TypedActorSystem::<StashOrderCommand>::create_from_props(&props, ActorSystemConfig::new(TestTickDriver::default()))
      .expect("system");
  let mut actor = system.user_guardian_ref();

  actor.tell(StashOrderCommand::Buffer(String::from("stashed")));
  actor.tell(StashOrderCommand::Open);
  actor.tell(StashOrderCommand::Marker(String::from("queued")));

  wait_until(|| read_stash_order_log(&mut actor).len() == 2);
  assert_eq!(read_stash_order_log(&mut actor), vec![String::from("buffer:stashed"), String::from("marker:queued")]);

  system.terminate().expect("terminate");
}

#[derive(Clone, Copy)]
struct LifecycleCommand;

#[test]
fn typed_behaviors_receive_signal_notifications() {
  let started = Arc::new(AtomicUsize::new(0));
  let post_stop = Arc::new(AtomicUsize::new(0));
  let start_probe = Arc::clone(&started);
  let post_stop_probe = Arc::clone(&post_stop);

  let props = TypedProps::<LifecycleCommand>::from_behavior_factory(move || {
    signal_probe_behavior(&start_probe, &post_stop_probe)
  });
  let system =
    TypedActorSystem::<LifecycleCommand>::create_from_props(&props, ActorSystemConfig::new(TestTickDriver::default()))
      .expect("system");
  system.terminate().expect("terminate");
  system.as_untyped().run_until_terminated(&SpinBlocker);

  assert_eq!(started.load(Ordering::SeqCst), 1);
  assert_eq!(post_stop.load(Ordering::SeqCst), 1);
}

#[derive(Clone)]
enum MismatchCommand {
  Trigger { reply_to: TypedActorRef<i32> },
}

#[derive(Clone, Copy)]
enum SupervisorCommand {
  CrashChild,
}

#[derive(Clone, Copy)]
enum ChildCommand {
  Crash,
}

#[derive(Clone)]
enum SchedulerProbeCommand {
  Check { reply_to: TypedActorRef<bool> },
}

struct MismatchActor;
struct SchedulerProbeActor;

impl TypedActor<MismatchCommand> for MismatchActor {
  fn receive(
    &mut self,
    _ctx: &mut TypedActorContext<'_, MismatchCommand>,
    message: &MismatchCommand,
  ) -> Result<(), ActorError> {
    match message {
      | MismatchCommand::Trigger { reply_to } => {
        let mut reply_to = reply_to.clone();
        reply_to.as_untyped_mut().tell(AnyMessage::new("unexpected".to_string()));
        Ok(())
      },
    }
  }
}

impl TypedActor<SchedulerProbeCommand> for SchedulerProbeActor {
  fn receive(
    &mut self,
    ctx: &mut TypedActorContext<'_, SchedulerProbeCommand>,
    message: &SchedulerProbeCommand,
  ) -> Result<(), ActorError> {
    match message {
      | SchedulerProbeCommand::Check { reply_to } => {
        let _ = ctx.system().scheduler();
        let mut reply_to = reply_to.clone();
        reply_to.tell(true);
        Ok(())
      },
    }
  }
}

#[test]
fn typed_ask_reports_type_mismatch() {
  let props = TypedProps::<MismatchCommand>::new(|| MismatchActor);
  let system =
    TypedActorSystem::<MismatchCommand>::create_from_props(&props, ActorSystemConfig::new(TestTickDriver::default()))
      .expect("system");
  let mut actor = system.user_guardian_ref();

  let response = actor.ask::<i32, _>(|reply_to| MismatchCommand::Trigger { reply_to });
  let mut future = response.future().clone();
  wait_until(|| future.is_ready());
  let result = future.try_take().expect("result");

  assert!(matches!(result, Err(TypedAskError::TypeMismatch)));

  system.terminate().expect("terminate");
}

#[test]
fn typed_context_exposes_scheduler() {
  let props = TypedProps::<SchedulerProbeCommand>::new(|| SchedulerProbeActor);
  let system = TypedActorSystem::<SchedulerProbeCommand>::create_from_props(
    &props,
    ActorSystemConfig::new(TestTickDriver::default()),
  )
  .expect("system");
  let mut actor = system.user_guardian_ref();

  let response = actor.ask::<bool, _>(|reply_to| SchedulerProbeCommand::Check { reply_to });
  let mut future = response.future().clone();
  wait_until(|| future.is_ready());
  let result = future.try_take().expect("result").expect("payload");

  assert!(result);

  system.terminate().expect("terminate");
}

fn wait_until(mut condition: impl FnMut() -> bool) {
  for _ in 0..10_000 {
    if condition() {
      return;
    }
    spin_loop();
  }
  assert!(condition());
}

fn wait_for(mut condition: impl FnMut() -> bool) -> bool {
  for _ in 0..10_000 {
    if condition() {
      return true;
    }
    spin_loop();
  }
  condition()
}

fn behavior_counter(total: i32) -> Behavior<CounterMessage> {
  Behaviors::receive_message(move |_ctx, message| match message {
    | CounterMessage::Increment(delta) => Ok(behavior_counter(total + delta)),
    | CounterMessage::Get { reply_to } => {
      let mut reply_to = reply_to.clone();
      reply_to.tell(total);
      Ok(Behaviors::same())
    },
  })
}

fn ignore_gate(total: u32) -> Behavior<IgnoreCommand> {
  Behaviors::receive_message(move |_ctx, message| match message {
    | IgnoreCommand::Add(delta) => Ok(ignore_gate(total + delta)),
    | IgnoreCommand::Reject => Ok(Behaviors::ignore()),
    | IgnoreCommand::Read { reply_to } => {
      let mut reply_to = reply_to.clone();
      reply_to.tell(total);
      Ok(Behaviors::same())
    },
  })
}

fn stash_behavior(total: u32) -> Behavior<StashCommand> {
  Behaviors::with_stash(32, move |stash| stash_locked_behavior(total, stash))
}

fn adapter_stash_behavior(total: u32, slot: &Arc<SpinSyncMutex<Option<TypedActorRef<i32>>>>) -> Behavior<StashCommand> {
  let slot = Arc::clone(slot);
  Behaviors::setup(move |ctx| {
    let adapter = ctx
      .message_adapter(|value: i32| {
        u32::try_from(value)
          .map(StashCommand::Buffer)
          .map_err(|_| AdapterError::Custom("negative value is not supported".into()))
      })
      .expect("register adapter");
    slot.lock().replace(adapter);
    stash_locked_behavior(total, StashBuffer::new(32))
  })
}

fn stash_behavior_with_capacity_limit(total: u32, overflow_counter: Arc<AtomicUsize>) -> Behavior<StashCommand> {
  Behaviors::with_stash(1, move |stash| stash_limited_locked_behavior(total, Arc::clone(&overflow_counter), stash))
}

fn stash_locked_behavior(total: u32, stash: StashBuffer<StashCommand>) -> Behavior<StashCommand> {
  Behaviors::receive_message(move |ctx, message| match message {
    | StashCommand::Buffer(_) => {
      stash.stash(ctx)?;
      Ok(Behaviors::same())
    },
    | StashCommand::Open => {
      let _ = stash.unstash_all(ctx)?;
      Ok(stash_open_behavior(total))
    },
    | StashCommand::Read { reply_to } => {
      let mut reply_to = reply_to.clone();
      reply_to.tell(total);
      Ok(Behaviors::same())
    },
  })
}

fn stash_limited_locked_behavior(
  total: u32,
  overflow_counter: Arc<AtomicUsize>,
  stash: StashBuffer<StashCommand>,
) -> Behavior<StashCommand> {
  Behaviors::receive_message(move |ctx, message| match message {
    | StashCommand::Buffer(_) => {
      if stash.is_full(ctx)? {
        overflow_counter.fetch_add(1, Ordering::SeqCst);
        return Ok(Behaviors::same());
      }
      stash.stash(ctx)?;
      Ok(Behaviors::same())
    },
    | StashCommand::Open => {
      let _ = stash.unstash_all(ctx)?;
      Ok(stash_open_behavior(total))
    },
    | StashCommand::Read { reply_to } => {
      let mut reply_to = reply_to.clone();
      reply_to.tell(total);
      Ok(Behaviors::same())
    },
  })
}

fn stash_open_behavior(total: u32) -> Behavior<StashCommand> {
  Behaviors::receive_message(move |_ctx, message| match message {
    | StashCommand::Buffer(delta) => Ok(stash_open_behavior(total + delta)),
    | StashCommand::Open => Ok(Behaviors::same()),
    | StashCommand::Read { reply_to } => {
      let mut reply_to = reply_to.clone();
      reply_to.tell(total);
      Ok(Behaviors::same())
    },
  })
}

fn stash_order_behavior(history: Vec<String>) -> Behavior<StashOrderCommand> {
  Behaviors::with_stash(32, move |stash| stash_order_locked_behavior(history.clone(), stash))
}

fn stash_order_locked_behavior(
  history: Vec<String>,
  stash: StashBuffer<StashOrderCommand>,
) -> Behavior<StashOrderCommand> {
  Behaviors::receive_message(move |ctx, message| match message {
    | StashOrderCommand::Buffer(_) | StashOrderCommand::Marker(_) => {
      stash.stash(ctx)?;
      Ok(Behaviors::same())
    },
    | StashOrderCommand::Open => {
      let _ = stash.unstash_all(ctx)?;
      Ok(stash_order_open_behavior(history.clone()))
    },
    | StashOrderCommand::Read { reply_to } => {
      let mut reply_to = reply_to.clone();
      reply_to.tell(history.clone());
      Ok(Behaviors::same())
    },
  })
}

fn stash_order_open_behavior(history: Vec<String>) -> Behavior<StashOrderCommand> {
  Behaviors::receive_message(move |_ctx, message| match message {
    | StashOrderCommand::Buffer(value) => {
      let mut next = history.clone();
      next.push(format!("buffer:{value}"));
      Ok(stash_order_open_behavior(next))
    },
    | StashOrderCommand::Marker(value) => {
      let mut next = history.clone();
      next.push(format!("marker:{value}"));
      Ok(stash_order_open_behavior(next))
    },
    | StashOrderCommand::Open => Ok(Behaviors::same()),
    | StashOrderCommand::Read { reply_to } => {
      let mut reply_to = reply_to.clone();
      reply_to.tell(history.clone());
      Ok(Behaviors::same())
    },
  })
}

fn signal_probe_behavior(started: &Arc<AtomicUsize>, post_stop: &Arc<AtomicUsize>) -> Behavior<LifecycleCommand> {
  let start_probe = Arc::clone(started);
  let post_stop_probe = Arc::clone(post_stop);
  Behaviors::setup(move |_ctx| {
    start_probe.fetch_add(1, Ordering::SeqCst);
    let post_stop_probe = post_stop_probe.clone();
    Behaviors::receive_signal(move |_ctx, signal| {
      match signal {
        | BehaviorSignal::PostStop => {
          post_stop_probe.fetch_add(1, Ordering::SeqCst);
        },
        | BehaviorSignal::Terminated(_)
        | BehaviorSignal::MessageAdaptionFailure(_)
        | BehaviorSignal::ChildFailed(_)
        | BehaviorSignal::PreRestart
        | BehaviorSignal::PostRestart => {},
      }
      Ok(Behaviors::same())
    })
  })
}

fn child_behavior(counter: &Arc<AtomicUsize>) -> Behavior<ChildCommand> {
  let start_probe = Arc::clone(counter);
  Behaviors::setup(move |_ctx| {
    start_probe.fetch_add(1, Ordering::SeqCst);
    Behaviors::receive_message(move |_ctx, message| match message {
      | ChildCommand::Crash => Err(ActorError::recoverable("boom")),
    })
  })
}

fn child_props(counter: &Arc<AtomicUsize>) -> TypedProps<ChildCommand> {
  let counter = Arc::clone(counter);
  TypedProps::from_behavior_factory(move || child_behavior(&counter))
}

struct PassThroughInterceptor {
  counter: Arc<AtomicUsize>,
}

impl BehaviorInterceptor<ChildCommand> for PassThroughInterceptor {
  fn around_receive(
    &mut self,
    context: &mut TypedActorContext<'_, ChildCommand>,
    message: &ChildCommand,
    target: &mut dyn FnMut(
      &mut TypedActorContext<'_, ChildCommand>,
      &ChildCommand,
    ) -> Result<Behavior<ChildCommand>, ActorError>,
  ) -> Result<Behavior<ChildCommand>, ActorError> {
    self.counter.fetch_add(1, Ordering::SeqCst);
    target(context, message)
  }
}

fn intercepted_child_props(
  counter: &Arc<AtomicUsize>,
  interceptor_counter: &Arc<AtomicUsize>,
) -> TypedProps<ChildCommand> {
  let counter = Arc::clone(counter);
  let interceptor_counter = Arc::clone(interceptor_counter);
  TypedProps::from_behavior_factory(move || {
    let interceptor_counter = Arc::clone(&interceptor_counter);
    Behaviors::intercept_behavior(
      move || Box::new(PassThroughInterceptor { counter: Arc::clone(&interceptor_counter) }),
      child_behavior(&counter),
    )
  })
}

fn supervised_parent_behavior(child: TypedProps<ChildCommand>) -> Behavior<SupervisorCommand> {
  Behaviors::setup(move |ctx| {
    let child_ref = ctx.spawn_child(&child).expect("spawn child");
    let handle = child_ref.actor_ref();
    Behaviors::receive_message(move |_ctx, message| match message {
      | SupervisorCommand::CrashChild => {
        handle.clone().tell(ChildCommand::Crash);
        Ok(Behaviors::same())
      },
    })
  })
}

fn supervised_parent_props(
  strategy: SupervisorStrategy,
  child: TypedProps<ChildCommand>,
) -> TypedProps<SupervisorCommand> {
  TypedProps::from_behavior_factory(move || {
    let behavior = supervised_parent_behavior(child.clone());
    Behaviors::supervise(behavior).on_failure(strategy.clone())
  })
}

#[test]
fn behaviors_supervise_restarts_children() {
  let start_counter = Arc::new(AtomicUsize::new(0));
  let child = child_props(&start_counter);
  let restart_strategy = SupervisorStrategy::new(
    SupervisorStrategyKind::OneForOne,
    RestartLimit::WithinWindow(5),
    Duration::from_secs(1),
    |_| SupervisorDirective::Restart,
  );
  let parent_props = supervised_parent_props(restart_strategy, child);
  let system = TypedActorSystem::<SupervisorCommand>::create_from_props(
    &parent_props,
    ActorSystemConfig::new(TestTickDriver::default()),
  )
  .expect("system");
  let mut parent = system.user_guardian_ref();

  wait_until(|| start_counter.load(Ordering::SeqCst) == 1);

  parent.tell(SupervisorCommand::CrashChild);

  wait_until(|| start_counter.load(Ordering::SeqCst) >= 2);

  system.terminate().expect("terminate");
}

#[test]
fn intercepted_behavior_survives_supervised_restart() {
  let start_counter = Arc::new(AtomicUsize::new(0));
  let interceptor_counter = Arc::new(AtomicUsize::new(0));
  let child = intercepted_child_props(&start_counter, &interceptor_counter);
  let restart_strategy = SupervisorStrategy::new(
    SupervisorStrategyKind::OneForOne,
    RestartLimit::WithinWindow(5),
    Duration::from_secs(1),
    |_| SupervisorDirective::Restart,
  );
  let parent_props = supervised_parent_props(restart_strategy, child);
  let system = TypedActorSystem::<SupervisorCommand>::create_from_props(
    &parent_props,
    ActorSystemConfig::new(TestTickDriver::default()),
  )
  .expect("system");
  let mut parent = system.user_guardian_ref();

  wait_until(|| start_counter.load(Ordering::SeqCst) == 1);

  parent.tell(SupervisorCommand::CrashChild);
  wait_until(|| interceptor_counter.load(Ordering::SeqCst) >= 1);

  wait_until(|| start_counter.load(Ordering::SeqCst) >= 2);

  parent.tell(SupervisorCommand::CrashChild);
  wait_until(|| interceptor_counter.load(Ordering::SeqCst) >= 2);
  wait_until(|| start_counter.load(Ordering::SeqCst) >= 3);

  system.terminate().expect("terminate");
}

#[test]
fn behaviors_supervise_stops_children() {
  let start_counter = Arc::new(AtomicUsize::new(0));
  let child = child_props(&start_counter);
  let stop_strategy = SupervisorStrategy::new(
    SupervisorStrategyKind::OneForOne,
    RestartLimit::WithinWindow(1),
    Duration::from_secs(1),
    |_| SupervisorDirective::Stop,
  );
  let parent_props = supervised_parent_props(stop_strategy, child);
  let system = TypedActorSystem::<SupervisorCommand>::create_from_props(
    &parent_props,
    ActorSystemConfig::new(TestTickDriver::default()),
  )
  .expect("system");
  let mut parent = system.user_guardian_ref();

  wait_until(|| start_counter.load(Ordering::SeqCst) == 1);

  parent.tell(SupervisorCommand::CrashChild);

  // 子アクターは再起動しないはずなので、カウンターが 1 のままであることを短期間検証する。
  for _ in 0..1_000 {
    assert_eq!(start_counter.load(Ordering::SeqCst), 1);
    spin_loop();
  }

  system.terminate().expect("terminate");
}

#[test]
fn backoff_strategy_via_supervise_on_failure() {
  let start_counter = Arc::new(AtomicUsize::new(0));
  let child = child_props(&start_counter);
  let backoff = BackoffSupervisorStrategy::new(Duration::from_millis(100), Duration::from_secs(1), 0.2);
  let parent_props = TypedProps::from_behavior_factory(move || {
    let behavior = supervised_parent_behavior(child.clone());
    Behaviors::supervise(behavior).on_failure(backoff.clone())
  });
  let system = TypedActorSystem::<SupervisorCommand>::create_from_props(
    &parent_props,
    ActorSystemConfig::new(TestTickDriver::default()),
  )
  .expect("system");
  let mut parent = system.user_guardian_ref();

  wait_until(|| start_counter.load(Ordering::SeqCst) == 1);

  parent.tell(SupervisorCommand::CrashChild);

  // バックオフ戦略では、回復可能なエラー時に子アクターが再起動されることを確認する。
  wait_until(|| start_counter.load(Ordering::SeqCst) >= 2);

  system.terminate().expect("terminate");
}

#[test]
fn backoff_strategy_stores_config_in_behavior() {
  let backoff = BackoffSupervisorStrategy::new(Duration::from_millis(100), Duration::from_secs(1), 0.2);
  let behavior: Behavior<ChildCommand> =
    Behaviors::supervise(Behaviors::receive_message(|_ctx, _msg: &ChildCommand| Ok(Behaviors::same())))
      .on_failure(backoff);
  // Behavior が Backoff 設定バリアントを保持していることを検証する。
  let config = behavior.supervisor_override().expect("supervisor override must be set");
  assert!(matches!(config, SupervisorStrategyConfig::Backoff(_)));
}

fn adapter_counter_behavior(
  slot: &Arc<SpinSyncMutex<Option<TypedActorRef<String>>>>,
) -> Behavior<AdapterCounterCommand> {
  let slot = Arc::clone(slot);
  Behaviors::setup(move |ctx| {
    let adapter = ctx
      .message_adapter(|value: String| {
        value.parse::<i32>().map(AdapterCounterCommand::Set).map_err(|_| AdapterError::Custom("parse error".into()))
      })
      .expect("register adapter");
    slot.lock().replace(adapter);
    counter_behavior(0)
  })
}

fn counter_behavior(value: i32) -> Behavior<AdapterCounterCommand> {
  Behaviors::receive_message(move |_ctx, message| match message {
    | AdapterCounterCommand::Set(delta) => Ok(counter_behavior(value + delta)),
    | AdapterCounterCommand::Read { reply_to } => {
      let mut reply_to = reply_to.clone();
      reply_to.tell(value);
      Ok(Behaviors::same())
    },
  })
}

#[test]
fn message_adapter_converts_external_messages() {
  let adapter_slot: Arc<SpinSyncMutex<Option<TypedActorRef<String>>>> = Arc::new(SpinSyncMutex::new(None));
  let props = TypedProps::<AdapterCounterCommand>::from_behavior_factory({
    let slot = adapter_slot.clone();
    move || adapter_counter_behavior(&slot)
  });
  let system = TypedActorSystem::<AdapterCounterCommand>::create_from_props(
    &props,
    ActorSystemConfig::new(TestTickDriver::default()),
  )
  .expect("system");
  let mut actor = system.user_guardian_ref();

  assert!(wait_for(|| adapter_slot.lock().is_some()), "adapter never registered");
  let mut adapter = adapter_slot.lock().clone().expect("adapter available");

  adapter.tell("5".to_string());
  adapter.tell("3".to_string());

  wait_until(|| read_counter_value(&mut actor) == 8);
  let value = read_counter_value(&mut actor);
  assert_eq!(value, 8);

  system.terminate().expect("terminate");
}

#[test]
fn adapter_not_found_routes_to_dead_letter() {
  let props = TypedProps::<AdapterCounterCommand>::from_behavior_factory(|| {
    Behaviors::setup(|ctx| {
      ctx
        .message_adapter(|value: String| {
          value.parse::<i32>().map(AdapterCounterCommand::Set).map_err(|_| AdapterError::Custom("parse error".into()))
        })
        .expect("register adapter");
      counter_behavior(0)
    })
  });
  let system = TypedActorSystem::<AdapterCounterCommand>::create_from_props(
    &props,
    ActorSystemConfig::new(TestTickDriver::default()),
  )
  .expect("system");
  let actor = system.user_guardian_ref();
  let mut untyped = actor.as_untyped().clone();

  let payload = AdapterPayload::new(7_u64);
  let envelope = AdapterEnvelope::new(payload, None);
  untyped.tell(AnyMessage::new(envelope));

  wait_until(|| !system.dead_letter_entries().is_empty());
  let entries: Vec<DeadLetterEntry> = system.dead_letter_entries();
  assert!(entries.iter().any(|entry| entry.reason() == DeadLetterReason::ExplicitRouting));

  system.terminate().expect("terminate");
}

#[test]
fn pipe_to_self_converts_messages_via_adapter() {
  let props = TypedProps::<AdapterCounterCommand>::from_behavior_factory(|| {
    Behaviors::setup(|ctx| {
      ctx
        .pipe_to_self(
          async { Ok::<_, ()>("6".to_string()) },
          |value: String| {
            value.parse::<i32>().map(AdapterCounterCommand::Set).map_err(|_| AdapterError::Custom("parse error".into()))
          },
          |_| Err(AdapterError::Custom("pipe failure".into())),
        )
        .expect("pipe");
      counter_behavior(0)
    })
  });
  let system = TypedActorSystem::<AdapterCounterCommand>::create_from_props(
    &props,
    ActorSystemConfig::new(TestTickDriver::default()),
  )
  .expect("system");
  let mut actor = system.user_guardian_ref();
  wait_until(|| read_counter_value(&mut actor) == 6);
  system.terminate().expect("terminate");
}

fn read_counter_value(actor: &mut TypedActorRef<AdapterCounterCommand>) -> i32 {
  let response = actor.ask::<i32, _>(|reply_to| AdapterCounterCommand::Read { reply_to });
  let mut future = response.future().clone();
  wait_until(|| future.is_ready());
  future.try_take().expect("result").expect("payload")
}

fn read_stash_total(actor: &mut TypedActorRef<StashCommand>) -> u32 {
  let response = actor.ask::<u32, _>(|reply_to| StashCommand::Read { reply_to });
  let mut future = response.future().clone();
  wait_until(|| future.is_ready());
  future.try_take().expect("result").expect("payload")
}

fn read_stash_order_log(actor: &mut TypedActorRef<StashOrderCommand>) -> Vec<String> {
  let response = actor.ask::<Vec<String>, _>(|reply_to| StashOrderCommand::Read { reply_to });
  let mut future = response.future().clone();
  wait_until(|| future.is_ready());
  future.try_take().expect("result").expect("payload")
}

fn assert_signal_type<T: Signal>() {}

fn assert_extensible_behavior_type<T: ExtensibleBehavior<LifecycleCommand>>() {}

fn assert_terminated_actor_ref_type(_actor_ref: &TypedActorRef<Infallible>) {}

#[test]
fn public_signal_types_implement_signal_marker_trait() {
  assert_signal_type::<BehaviorSignal>();
  assert_signal_type::<PreRestart>();
  assert_signal_type::<PostStop>();
  assert_signal_type::<Terminated>();
  assert_signal_type::<ChildFailed>();
  assert_signal_type::<MessageAdaptionFailure>();
}

#[test]
fn typed_root_reexports_extensible_behavior_trait() {
  assert_extensible_behavior_type::<RootExtensibleBehaviorProbe>();
}

#[test]
fn dedicated_signal_types_convert_into_behavior_signal_variants() {
  let system = TypedActorSystem::<()>::new_empty();
  let terminated_ref = system.ignore_ref::<Infallible>();
  let child_ref = system.ignore_ref::<Infallible>();
  let terminated = Terminated::new(terminated_ref.clone());
  let child_failed = ChildFailed::new(child_ref.clone(), ActorError::recoverable("boom"));

  assert_terminated_actor_ref_type(terminated.actor_ref());
  assert_terminated_actor_ref_type(child_failed.actor_ref());

  assert_eq!(BehaviorSignal::from(PreRestart), BehaviorSignal::PreRestart);
  assert_eq!(BehaviorSignal::from(PostStop), BehaviorSignal::PostStop);
  assert_eq!(BehaviorSignal::from(terminated.clone()), { BehaviorSignal::Terminated(terminated) });
  assert_eq!(BehaviorSignal::from(child_failed.clone()), BehaviorSignal::ChildFailed(child_failed),);
  assert_eq!(
    BehaviorSignal::from(MessageAdaptionFailure::new(AdapterError::Custom(String::from("bad adapter")))),
    BehaviorSignal::MessageAdaptionFailure(MessageAdaptionFailure::new(AdapterError::Custom(String::from(
      "bad adapter",
    )))),
  );
}
