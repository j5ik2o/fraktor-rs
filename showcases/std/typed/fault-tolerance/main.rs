#![cfg(not(target_os = "none"))]

use core::time::Duration;
use std::thread;

use fraktor_actor_adaptor_std_rs::std::{StdBlocker, tick_driver::StdTickDriver};
use fraktor_actor_core_rs::core::{
  kernel::actor::{error::ActorError, setup::ActorSystemConfig},
  typed::{Behavior, SupervisorStrategy, TypedActorSystem, TypedProps, dsl::Behaviors},
};
use fraktor_utils_core_rs::core::sync::{SharedLock, SpinSyncMutex};

#[derive(Clone, Copy)]
enum ParentCommand {
  Start,
}

#[derive(Clone, Copy)]
enum ChildCommand {
  Crash,
  Work,
}

fn fragile_child(events: SharedLock<Vec<&'static str>>) -> Behavior<ChildCommand> {
  Behaviors::setup(move |_ctx| {
    events.with_lock(|events| events.push("child-started"));
    let events = events.clone();
    Behaviors::receive_message(move |_ctx, message: &ChildCommand| match message {
      | ChildCommand::Crash => Err(ActorError::recoverable("typed fault-tolerance example crash")),
      | ChildCommand::Work => {
        events.with_lock(|events| events.push("work-after-restart"));
        Ok(Behaviors::same())
      },
    })
  })
}

fn parent(events: SharedLock<Vec<&'static str>>) -> Behavior<ParentCommand> {
  let inner = Behaviors::setup(move |ctx| {
    let child = ctx
      .spawn_child_watched(&TypedProps::from_behavior_factory({
        let events = events.clone();
        move || fragile_child(events.clone())
      }))
      .expect("spawn supervised child");
    let child_ref = child.actor_ref();

    Behaviors::receive_message(move |_ctx, message: &ParentCommand| {
      if matches!(message, ParentCommand::Start) {
        let mut child_ref = child_ref.clone();
        child_ref.try_tell(ChildCommand::Crash).map_err(|error| ActorError::recoverable(format!("{error:?}")))?;
        child_ref.try_tell(ChildCommand::Work).map_err(|error| ActorError::recoverable(format!("{error:?}")))?;
      }
      Ok(Behaviors::same())
    })
  });

  Behaviors::supervise(inner).on_failure(SupervisorStrategy::restart())
}

fn main() {
  let events = SharedLock::new_with_driver::<SpinSyncMutex<_>>(Vec::new());
  let props = TypedProps::from_behavior_factory({
    let events = events.clone();
    move || parent(events.clone())
  });
  let system =
    TypedActorSystem::create_with_config(&props, ActorSystemConfig::new(StdTickDriver::default())).expect("system");
  let termination = system.when_terminated();
  let mut guardian = system.user_guardian_ref();

  guardian.tell(ParentCommand::Start);
  wait_until(|| events.with_lock(|events| events.contains(&"work-after-restart")));
  assert!(events.with_lock(|events| events.iter().filter(|event| **event == "child-started").count() >= 2));

  system.terminate().expect("terminate");
  termination.wait_blocking(&StdBlocker::new());
}

fn wait_until(mut condition: impl FnMut() -> bool) {
  for _ in 0..1_000 {
    if condition() {
      return;
    }
    thread::sleep(Duration::from_millis(1));
  }
  assert!(condition());
}
