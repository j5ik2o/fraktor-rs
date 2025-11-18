//! Demonstrates wrapping typed behaviors with `Behaviors::supervise` to control child failures.

#[path = "../std_tick_driver_support.rs"]
mod std_tick_driver_support;

use std::{
  sync::atomic::{AtomicUsize, Ordering},
  thread,
  time::Duration,
};

use fraktor_actor_rs::{
  core::{
    error::ActorError,
    supervision::{SupervisorDirective, SupervisorStrategy, SupervisorStrategyKind},
  },
  std::typed::{Behavior, BehaviorSignal, Behaviors, TypedActorSystem, TypedProps},
};
use fraktor_utils_rs::core::sync::ArcShared;

#[derive(Clone, Copy)]
enum GuardianCommand {
  CrashWorker,
}

#[derive(Clone, Copy)]
enum WorkerCommand {
  Crash,
}

fn worker(counter: ArcShared<AtomicUsize>) -> Behavior<WorkerCommand> {
  let starts = ArcShared::clone(&counter);
  Behaviors::receive_message(move |_ctx, message| match message {
    | WorkerCommand::Crash => Err(ActorError::recoverable("simulated failure")),
  })
  .receive_signal(move |_ctx, signal| {
    if matches!(signal, BehaviorSignal::Started) {
      let current = starts.fetch_add(1, Ordering::SeqCst) + 1;
      println!("worker restarted {} time(s)", current);
    }
    Ok(Behaviors::same())
  })
}

fn guardian(counter: ArcShared<AtomicUsize>) -> Behavior<GuardianCommand> {
  let worker_props = {
    let counter = ArcShared::clone(&counter);
    TypedProps::from_behavior_factory(move || worker(ArcShared::clone(&counter)))
  };

  let behavior = Behaviors::setup(move |ctx| {
    let child = ctx.spawn_child(&worker_props).expect("spawn worker");
    let child_ref = child.actor_ref();
    println!("guardian spawned worker {:?}", child_ref.pid());

    Behaviors::receive_message(move |_ctx, message| match message {
      | GuardianCommand::CrashWorker => {
        println!("guardian triggering worker crash");
        child_ref.tell(WorkerCommand::Crash).expect("send crash");
        Ok(Behaviors::same())
      },
    })
  });

  let strategy =
    SupervisorStrategy::new(SupervisorStrategyKind::OneForOne, 5, Duration::from_secs(1), |error| match error {
      | ActorError::Recoverable(_) => SupervisorDirective::Restart,
      | ActorError::Fatal(_) => SupervisorDirective::Stop,
    });

  Behaviors::supervise(behavior).on_failure(strategy)
}

#[allow(clippy::print_stdout)]
fn main() {
  let counter = ArcShared::new(AtomicUsize::new(0));
  let props = {
    let counter = ArcShared::clone(&counter);
    TypedProps::from_behavior_factory(move || guardian(ArcShared::clone(&counter)))
  };

  let tick_driver = std_tick_driver_support::hardware_tick_driver_config();
  let system = TypedActorSystem::new(&props, tick_driver).expect("typed system");
  let guardian_ref = system.user_guardian_ref();

  guardian_ref.tell(GuardianCommand::CrashWorker).expect("first crash");
  thread::sleep(Duration::from_millis(20));
  guardian_ref.tell(GuardianCommand::CrashWorker).expect("second crash");
  thread::sleep(Duration::from_millis(20));

  println!("worker observed {} start events", counter.load(Ordering::SeqCst));

  system.terminate().expect("terminate");
  let termination = system.when_terminated();
  while !termination.is_ready() {
    thread::sleep(Duration::from_millis(10));
  }
}
