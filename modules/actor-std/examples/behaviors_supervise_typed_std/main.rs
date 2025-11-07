//! Demonstrates wrapping typed behaviors with `Behaviors::supervise` to control child failures.

use std::{
  sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
  },
  thread,
  time::Duration,
};

use cellactor_actor_core_rs::{
  error::ActorError,
  supervision::{SupervisorDirective, SupervisorStrategy, SupervisorStrategyKind},
};
use cellactor_actor_std_rs::typed::{Behavior, BehaviorSignal, Behaviors, TypedActorSystem, TypedProps};

#[derive(Clone, Copy)]
enum GuardianCommand {
  CrashWorker,
}

#[derive(Clone, Copy)]
enum WorkerCommand {
  Crash,
}

fn worker(counter: Arc<AtomicUsize>) -> Behavior<WorkerCommand> {
  let starts = Arc::clone(&counter);
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

fn guardian(counter: Arc<AtomicUsize>) -> Behavior<GuardianCommand> {
  let worker_props = {
    let counter = Arc::clone(&counter);
    TypedProps::from_behavior_factory(move || worker(Arc::clone(&counter)))
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
  let counter = Arc::new(AtomicUsize::new(0));
  let props = {
    let counter = Arc::clone(&counter);
    TypedProps::from_behavior_factory(move || guardian(Arc::clone(&counter)))
  };

  let system = TypedActorSystem::new(&props).expect("typed system");
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
