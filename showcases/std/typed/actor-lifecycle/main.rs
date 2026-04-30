#![cfg(not(target_os = "none"))]

use core::time::Duration;
use std::thread;

use fraktor_actor_adaptor_std_rs::std::{StdBlocker, tick_driver::StdTickDriver};
use fraktor_actor_core_rs::core::{
  kernel::actor::{error::ActorError, setup::ActorSystemConfig},
  typed::{Behavior, TypedActorSystem, TypedProps, dsl::Behaviors, message_and_signals::BehaviorSignal},
};
use fraktor_utils_core_rs::core::sync::{SharedLock, SpinSyncMutex};

#[derive(Clone, Copy)]
enum ParentCommand {
  StopChild,
}

#[derive(Clone, Copy)]
enum ChildCommand {}

fn child(events: SharedLock<Vec<&'static str>>) -> Behavior<ChildCommand> {
  Behaviors::receive_message(|_ctx, _message: &ChildCommand| Ok(Behaviors::same())).receive_signal(
    move |_ctx, signal| {
      if matches!(signal, BehaviorSignal::PostStop) {
        events.with_lock(|events| events.push("child-post-stop"));
      }
      Ok(Behaviors::same())
    },
  )
}

fn parent(events: SharedLock<Vec<&'static str>>) -> Behavior<ParentCommand> {
  Behaviors::setup(move |ctx| {
    events.with_lock(|events| events.push("parent-setup"));
    let child_ref = ctx
      .spawn_child_watched(&TypedProps::from_behavior_factory({
        let events = events.clone();
        move || child(events.clone())
      }))
      .expect("spawn watched child");

    Behaviors::receive_message(move |ctx, message: &ParentCommand| {
      if matches!(message, ParentCommand::StopChild) {
        ctx.stop_child(&child_ref).map_err(|error| ActorError::recoverable(format!("{error:?}")))?;
      }
      Ok(Behaviors::same())
    })
    .receive_signal({
      let events = events.clone();
      move |_ctx, signal| {
        if matches!(signal, BehaviorSignal::Terminated(_)) {
          events.with_lock(|events| events.push("parent-observed-terminated"));
        }
        Ok(Behaviors::same())
      }
    })
  })
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

  guardian.tell(ParentCommand::StopChild);
  wait_until(|| events.with_lock(|events| events.contains(&"parent-observed-terminated")));
  let snapshot = events.with_lock(|events| events.clone());
  assert!(snapshot.contains(&"parent-setup"));
  assert!(snapshot.contains(&"child-post-stop"));

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
