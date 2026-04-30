#![cfg(not(target_os = "none"))]

use core::time::Duration;
use std::thread;

use fraktor_actor_adaptor_std_rs::std::{StdBlocker, tick_driver::StdTickDriver};
use fraktor_actor_core_rs::core::{
  kernel::{actor::setup::ActorSystemConfig, dispatch::dispatcher::DEFAULT_BLOCKING_DISPATCHER_ID},
  typed::{Behavior, TypedActorSystem, TypedProps, dsl::Behaviors},
};
use fraktor_utils_core_rs::core::sync::{SharedLock, SpinSyncMutex};

#[derive(Clone, Copy)]
enum Command {
  Run,
}

fn worker(events: SharedLock<Vec<&'static str>>) -> Behavior<Command> {
  Behaviors::receive_message(move |_ctx, message: &Command| {
    if matches!(message, Command::Run) {
      events.with_lock(|events| events.push("typed-blocking-dispatcher-work"));
    }
    Ok(Behaviors::same())
  })
}

fn main() {
  let events = SharedLock::new_with_driver::<SpinSyncMutex<_>>(Vec::new());
  let props = TypedProps::from_behavior_factory({
    let events = events.clone();
    move || worker(events.clone())
  })
  .with_dispatcher_from_config(DEFAULT_BLOCKING_DISPATCHER_ID);
  let system =
    TypedActorSystem::create_with_config(&props, ActorSystemConfig::new(StdTickDriver::default())).expect("system");
  let termination = system.when_terminated();
  let mut actor = system.user_guardian_ref();

  actor.tell(Command::Run);
  wait_until(|| events.with_lock(|events| events.as_slice() == ["typed-blocking-dispatcher-work"]));

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
