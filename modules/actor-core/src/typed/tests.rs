use alloc::{string::ToString, sync::Arc};
use core::{
  hint::spin_loop,
  sync::atomic::{AtomicUsize, Ordering},
  time::Duration,
};

use crate::{
  NoStdToolbox,
  error::ActorError,
  supervision::{SupervisorDirective, SupervisorStrategy, SupervisorStrategyKind},
  typed::{
    Behavior, BehaviorSignal, Behaviors, TypedAskError,
    actor_prim::{TypedActor, TypedActorContextGeneric},
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
  let termination = system.as_untyped().when_terminated();

  system.terminate().expect("terminate");
  wait_until(|| termination.is_ready());

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

struct MismatchActor;

impl TypedActor<MismatchCommand> for MismatchActor {
  fn receive(
    &mut self,
    ctx: &mut TypedActorContextGeneric<'_, MismatchCommand>,
    _message: &MismatchCommand,
  ) -> Result<(), ActorError> {
    ctx.reply("unexpected".to_string()).map_err(|error| ActorError::from_send_error(&error))
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

fn wait_until(mut condition: impl FnMut() -> bool) {
  for _ in 0..10_000 {
    if condition() {
      return;
    }
    spin_loop();
  }
  assert!(condition());
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
