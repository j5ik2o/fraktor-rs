use alloc::sync::Arc;
use core::{
  hint::spin_loop,
  sync::atomic::{AtomicUsize, Ordering},
};

use crate::{
  NoStdToolbox,
  error::ActorError,
  typed::{
    Behavior, BehaviorSignal, Behaviors,
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

  let response = counter.ask(CounterMessage::Get).expect("ask get");
  let future = response.future().clone();
  wait_until(|| future.is_ready());
  let reply = future.try_take().expect("reply available");
  let payload = reply.payload().downcast_ref::<i32>().copied().expect("payload downcast");

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

  let response = counter.ask(CounterMessage::Get).expect("ask get");
  let future = response.future().clone();
  wait_until(|| future.is_ready());
  let reply = future.try_take().expect("reply available");
  let payload = reply.payload().downcast_ref::<i32>().copied().expect("payload downcast");

  assert_eq!(payload, 8);

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
