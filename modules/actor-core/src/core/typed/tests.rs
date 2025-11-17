use alloc::{
  string::{String, ToString},
  sync::Arc,
};
use core::{
  hint::spin_loop,
  sync::atomic::{AtomicUsize, Ordering},
  time::Duration,
};

use fraktor_utils_core_rs::core::runtime_toolbox::{NoStdMutex, NoStdToolbox};

use crate::core::{
  dead_letter::DeadLetterReason,
  error::ActorError,
  messaging::AnyMessageGeneric,
  supervision::{SupervisorDirective, SupervisorStrategy, SupervisorStrategyKind},
  typed::{
    Behavior, BehaviorSignal, Behaviors, TypedAskError,
    actor_prim::{TypedActor, TypedActorContextGeneric, TypedActorRef},
    message_adapter::{AdapterEnvelope, AdapterFailure, AdapterPayload},
    props::TypedPropsGeneric,
    system::TypedActorSystemGeneric,
  },
};

#[derive(Clone, Copy)]
enum CounterMessage {
  Increment(i32),
  Get,
}

#[derive(Clone, Copy)]
enum IgnoreCommand {
  Add(u32),
  Reject,
  Read,
}

#[derive(Clone, Copy)]
enum AdapterCounterCommand {
  Set(i32),
  Read,
}

struct CounterActor {
  total: i32,
}

impl CounterActor {
  const fn new() -> Self {
    Self { total: 0 }
  }
}

impl TypedActor<CounterMessage> for CounterActor {
  fn receive(
    &mut self,
    ctx: &mut TypedActorContextGeneric<'_, CounterMessage>,
    message: &CounterMessage,
  ) -> Result<(), ActorError> {
    match message {
      | CounterMessage::Increment(delta) => {
        self.total += delta;
        Ok(())
      },
      | CounterMessage::Get => {
        ctx.reply(self.total).map_err(|error| ActorError::from_send_error(&error))?;
        Ok(())
      },
    }
  }
}

#[test]
fn typed_actor_system_handles_basic_flow() {
  let props = TypedPropsGeneric::<CounterMessage, NoStdToolbox>::new(CounterActor::new);
  let system = TypedActorSystemGeneric::<CounterMessage, NoStdToolbox>::new(&props).expect("system");
  let counter = system.user_guardian_ref();

  counter.tell(CounterMessage::Increment(2)).expect("tell increment one");
  counter.tell(CounterMessage::Increment(5)).expect("tell increment two");

  let response = counter.ask::<i32>(CounterMessage::Get).expect("ask get");
  let future = response.future().clone();
  wait_until(|| future.is_ready());
  let payload = future.try_take().expect("reply available").expect("typed payload");

  assert_eq!(payload, 7);

  system.terminate().expect("terminate");
}

#[test]
fn typed_behaviors_handle_recursive_state() {
  let props = TypedPropsGeneric::<CounterMessage, NoStdToolbox>::from_behavior_factory(|| behavior_counter(0));
  let system = TypedActorSystemGeneric::<CounterMessage, NoStdToolbox>::new(&props).expect("system");
  let counter = system.user_guardian_ref();

  counter.tell(CounterMessage::Increment(3)).expect("increment one");
  counter.tell(CounterMessage::Increment(5)).expect("increment two");

  let response = counter.ask::<i32>(CounterMessage::Get).expect("ask get");
  let future = response.future().clone();
  wait_until(|| future.is_ready());
  let payload = future.try_take().expect("reply available").expect("typed payload");

  assert_eq!(payload, 8);

  system.terminate().expect("terminate");
}

#[test]
fn typed_behaviors_ignore_keeps_current_state() {
  let props = TypedPropsGeneric::<IgnoreCommand, NoStdToolbox>::from_behavior_factory(|| ignore_gate(0));
  let system = TypedActorSystemGeneric::<IgnoreCommand, NoStdToolbox>::new(&props).expect("system");
  let gate = system.user_guardian_ref();

  gate.tell(IgnoreCommand::Add(1)).expect("add before reject");
  gate.tell(IgnoreCommand::Reject).expect("reject once");
  gate.tell(IgnoreCommand::Add(5)).expect("add after reject");

  let response = gate.ask::<u32>(IgnoreCommand::Read).expect("ask read");
  let future = response.future().clone();
  wait_until(|| future.is_ready());
  let payload = future.try_take().expect("reply available").expect("typed payload");

  assert_eq!(payload, 6);

  system.terminate().expect("terminate");
}

#[derive(Clone, Copy)]
struct LifecycleCommand;

#[test]
fn typed_behaviors_receive_signal_notifications() {
  let started = Arc::new(AtomicUsize::new(0));
  let stopped = Arc::new(AtomicUsize::new(0));
  let start_probe = Arc::clone(&started);
  let stop_probe = Arc::clone(&stopped);

  let props = TypedPropsGeneric::<LifecycleCommand, NoStdToolbox>::from_behavior_factory(move || {
    signal_probe_behavior(&start_probe, &stop_probe)
  });
  let system = TypedActorSystemGeneric::<LifecycleCommand, NoStdToolbox>::new(&props).expect("system");
  system.terminate().expect("terminate");
  system.as_untyped().run_until_terminated();

  assert_eq!(started.load(Ordering::SeqCst), 1);
  assert_eq!(stopped.load(Ordering::SeqCst), 1);
}

#[derive(Clone, Copy)]
enum MismatchCommand {
  Trigger,
}

#[derive(Clone, Copy)]
enum SupervisorCommand {
  CrashChild,
}

#[derive(Clone, Copy)]
enum ChildCommand {
  Crash,
}

#[derive(Clone, Copy)]
enum SchedulerProbeCommand {
  Check,
}

struct MismatchActor;
struct SchedulerProbeActor;

impl TypedActor<MismatchCommand> for MismatchActor {
  fn receive(
    &mut self,
    ctx: &mut TypedActorContextGeneric<'_, MismatchCommand>,
    _message: &MismatchCommand,
  ) -> Result<(), ActorError> {
    ctx.reply("unexpected".to_string()).map_err(|error| ActorError::from_send_error(&error))
  }
}

impl TypedActor<SchedulerProbeCommand> for SchedulerProbeActor {
  fn receive(
    &mut self,
    ctx: &mut TypedActorContextGeneric<'_, SchedulerProbeCommand>,
    message: &SchedulerProbeCommand,
  ) -> Result<(), ActorError> {
    match message {
      | SchedulerProbeCommand::Check => {
        let has_context = ctx.system().scheduler_context().is_some();
        ctx.reply(has_context).map_err(|error| ActorError::from_send_error(&error))
      },
    }
  }
}

#[test]
fn typed_ask_reports_type_mismatch() {
  let props = TypedPropsGeneric::<MismatchCommand, NoStdToolbox>::new(|| MismatchActor);
  let system = TypedActorSystemGeneric::<MismatchCommand, NoStdToolbox>::new(&props).expect("system");
  let actor = system.user_guardian_ref();

  let response = actor.ask::<i32>(MismatchCommand::Trigger).expect("ask");
  let future = response.future().clone();
  wait_until(|| future.is_ready());
  let result = future.try_take().expect("result");

  assert!(matches!(result, Err(TypedAskError::TypeMismatch)));

  system.terminate().expect("terminate");
}

#[test]
fn typed_context_exposes_scheduler_context() {
  let props = TypedPropsGeneric::<SchedulerProbeCommand, NoStdToolbox>::new(|| SchedulerProbeActor);
  let system = TypedActorSystemGeneric::<SchedulerProbeCommand, NoStdToolbox>::new(&props).expect("system");
  let actor = system.user_guardian_ref();

  let response = actor.ask::<bool>(SchedulerProbeCommand::Check).expect("ask");
  let future = response.future().clone();
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

fn behavior_counter(total: i32) -> Behavior<CounterMessage, NoStdToolbox> {
  Behaviors::receive_message(move |ctx, message| match message {
    | CounterMessage::Increment(delta) => Ok(behavior_counter(total + delta)),
    | CounterMessage::Get => {
      ctx.reply(total).map_err(|error| ActorError::from_send_error(&error))?;
      Ok(Behaviors::same())
    },
  })
}

fn ignore_gate(total: u32) -> Behavior<IgnoreCommand, NoStdToolbox> {
  Behaviors::receive_message(move |ctx, message| match message {
    | IgnoreCommand::Add(delta) => Ok(ignore_gate(total + delta)),
    | IgnoreCommand::Reject => Ok(Behaviors::ignore()),
    | IgnoreCommand::Read => {
      ctx.reply(total).map_err(|error| ActorError::from_send_error(&error))?;
      Ok(Behaviors::same())
    },
  })
}

fn signal_probe_behavior(
  started: &Arc<AtomicUsize>,
  stopped: &Arc<AtomicUsize>,
) -> Behavior<LifecycleCommand, NoStdToolbox> {
  let start_probe = Arc::clone(started);
  let stop_probe = Arc::clone(stopped);
  Behaviors::receive_signal(move |_ctx, signal| {
    match signal {
      | BehaviorSignal::Started => {
        start_probe.fetch_add(1, Ordering::SeqCst);
      },
      | BehaviorSignal::Stopped => {
        stop_probe.fetch_add(1, Ordering::SeqCst);
      },
      | BehaviorSignal::Terminated(_) => {},
      | BehaviorSignal::AdapterFailed(_) => {},
    }
    Ok(Behaviors::same())
  })
}

fn child_behavior(counter: &Arc<AtomicUsize>) -> Behavior<ChildCommand, NoStdToolbox> {
  let start_probe = Arc::clone(counter);
  Behaviors::receive_message(move |_ctx, message| match message {
    | ChildCommand::Crash => Err(ActorError::recoverable("boom")),
  })
  .receive_signal(move |_ctx, signal| {
    if matches!(signal, BehaviorSignal::Started) {
      start_probe.fetch_add(1, Ordering::SeqCst);
    }
    Ok(Behaviors::same())
  })
}

fn child_props(counter: &Arc<AtomicUsize>) -> TypedPropsGeneric<ChildCommand, NoStdToolbox> {
  let counter = Arc::clone(counter);
  TypedPropsGeneric::from_behavior_factory(move || child_behavior(&counter))
}

fn supervised_parent_behavior(
  child: TypedPropsGeneric<ChildCommand, NoStdToolbox>,
) -> Behavior<SupervisorCommand, NoStdToolbox> {
  Behaviors::setup(move |ctx| {
    let child_ref = ctx.spawn_child(&child).expect("spawn child");
    let handle = child_ref.actor_ref();
    Behaviors::receive_message(move |_ctx, message| match message {
      | SupervisorCommand::CrashChild => {
        handle.tell(ChildCommand::Crash).expect("crash child");
        Ok(Behaviors::same())
      },
    })
  })
}

fn supervised_parent_props(
  strategy: SupervisorStrategy,
  child: TypedPropsGeneric<ChildCommand, NoStdToolbox>,
) -> TypedPropsGeneric<SupervisorCommand, NoStdToolbox> {
  TypedPropsGeneric::from_behavior_factory(move || {
    let behavior = supervised_parent_behavior(child.clone());
    Behaviors::supervise(behavior).on_failure(strategy.clone())
  })
}

#[test]
fn behaviors_supervise_restarts_children() {
  let start_counter = Arc::new(AtomicUsize::new(0));
  let child = child_props(&start_counter);
  let restart_strategy = SupervisorStrategy::new(SupervisorStrategyKind::OneForOne, 5, Duration::from_secs(1), |_| {
    SupervisorDirective::Restart
  });
  let parent_props = supervised_parent_props(restart_strategy, child);
  let system = TypedActorSystemGeneric::<SupervisorCommand, NoStdToolbox>::new(&parent_props).expect("system");
  let parent = system.user_guardian_ref();

  wait_until(|| start_counter.load(Ordering::SeqCst) == 1);

  parent.tell(SupervisorCommand::CrashChild).expect("crash");

  wait_until(|| start_counter.load(Ordering::SeqCst) >= 2);

  system.terminate().expect("terminate");
}

#[test]
fn behaviors_supervise_stops_children() {
  let start_counter = Arc::new(AtomicUsize::new(0));
  let child = child_props(&start_counter);
  let stop_strategy = SupervisorStrategy::new(SupervisorStrategyKind::OneForOne, 1, Duration::from_secs(1), |_| {
    SupervisorDirective::Stop
  });
  let parent_props = supervised_parent_props(stop_strategy, child);
  let system = TypedActorSystemGeneric::<SupervisorCommand, NoStdToolbox>::new(&parent_props).expect("system");
  let parent = system.user_guardian_ref();

  wait_until(|| start_counter.load(Ordering::SeqCst) == 1);

  parent.tell(SupervisorCommand::CrashChild).expect("crash");

  // Child should not restart, so validate the counter stays at 1 for a short period.
  for _ in 0..1_000 {
    assert_eq!(start_counter.load(Ordering::SeqCst), 1);
    spin_loop();
  }

  system.terminate().expect("terminate");
}

fn adapter_counter_behavior(
  slot: &Arc<NoStdMutex<Option<TypedActorRef<String>>>>,
) -> Behavior<AdapterCounterCommand, NoStdToolbox> {
  let slot = Arc::clone(slot);
  Behaviors::setup(move |ctx| {
    let adapter = ctx
      .message_adapter(|value: String| {
        value.parse::<i32>().map(AdapterCounterCommand::Set).map_err(|_| AdapterFailure::Custom("parse error".into()))
      })
      .expect("register adapter");
    slot.lock().replace(adapter);
    counter_behavior(0)
  })
}

fn counter_behavior(value: i32) -> Behavior<AdapterCounterCommand, NoStdToolbox> {
  Behaviors::receive_message(move |ctx, message| match message {
    | AdapterCounterCommand::Set(delta) => Ok(counter_behavior(value + delta)),
    | AdapterCounterCommand::Read => {
      ctx.reply(value).map_err(|error| ActorError::from_send_error(&error))?;
      Ok(Behaviors::same())
    },
  })
}

#[test]
fn message_adapter_converts_external_messages() {
  let adapter_slot: Arc<NoStdMutex<Option<TypedActorRef<String>>>> = Arc::new(NoStdMutex::new(None));
  let props = TypedPropsGeneric::<AdapterCounterCommand, NoStdToolbox>::from_behavior_factory({
    let slot = adapter_slot.clone();
    move || adapter_counter_behavior(&slot)
  });
  let system = TypedActorSystemGeneric::<AdapterCounterCommand, NoStdToolbox>::new(&props).expect("system");
  let actor = system.user_guardian_ref();

  assert!(wait_for(|| adapter_slot.lock().is_some()), "adapter never registered");
  let adapter = adapter_slot.lock().clone().expect("adapter available");

  adapter.tell("5".to_string()).expect("set one");
  adapter.tell("3".to_string()).expect("set two");

  wait_until(|| read_counter_value(&actor) == 8);
  let value = read_counter_value(&actor);
  assert_eq!(value, 8);

  system.terminate().expect("terminate");
}

#[test]
fn adapter_not_found_routes_to_dead_letter() {
  let props = TypedPropsGeneric::<AdapterCounterCommand, NoStdToolbox>::from_behavior_factory(|| {
    Behaviors::setup(|ctx| {
      ctx
        .message_adapter(|value: String| {
          value.parse::<i32>().map(AdapterCounterCommand::Set).map_err(|_| AdapterFailure::Custom("parse error".into()))
        })
        .expect("register adapter");
      counter_behavior(0)
    })
  });
  let system = TypedActorSystemGeneric::<AdapterCounterCommand, NoStdToolbox>::new(&props).expect("system");
  let actor = system.user_guardian_ref();
  let untyped = actor.as_untyped().clone();

  let payload = AdapterPayload::<NoStdToolbox>::new(7_u64);
  let envelope = AdapterEnvelope::new(payload, None);
  untyped.tell(AnyMessageGeneric::new(envelope)).expect("send envelope");

  wait_until(|| !system.dead_letters().is_empty());
  let entries = system.dead_letters();
  assert!(entries.iter().any(|entry| entry.reason() == DeadLetterReason::ExplicitRouting));

  system.terminate().expect("terminate");
}

#[test]
fn pipe_to_self_converts_messages_via_adapter() {
  let props = TypedPropsGeneric::<AdapterCounterCommand, NoStdToolbox>::from_behavior_factory(|| {
    Behaviors::setup(|ctx| {
      ctx
        .pipe_to_self(
          async { Ok::<_, ()>("6".to_string()) },
          |value: String| {
            value
              .parse::<i32>()
              .map(AdapterCounterCommand::Set)
              .map_err(|_| AdapterFailure::Custom("parse error".into()))
          },
          |_| Err(AdapterFailure::Custom("pipe failure".into())),
        )
        .expect("pipe");
      counter_behavior(0)
    })
  });
  let system = TypedActorSystemGeneric::<AdapterCounterCommand, NoStdToolbox>::new(&props).expect("system");
  let actor = system.user_guardian_ref();
  wait_until(|| read_counter_value(&actor) == 6);
  system.terminate().expect("terminate");
}

fn read_counter_value(actor: &TypedActorRef<AdapterCounterCommand>) -> i32 {
  let response = actor.ask::<i32>(AdapterCounterCommand::Read).expect("ask read");
  let future = response.future().clone();
  wait_until(|| future.is_ready());
  future.try_take().expect("result").expect("payload")
}
