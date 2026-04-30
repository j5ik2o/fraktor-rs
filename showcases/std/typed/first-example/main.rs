#![cfg(not(target_os = "none"))]

use core::time::Duration;
use std::thread;

use fraktor_actor_adaptor_std_rs::std::{StdBlocker, tick_driver::StdTickDriver};
use fraktor_actor_core_rs::core::{
  kernel::actor::setup::ActorSystemConfig,
  typed::{Behavior, TypedActorSystem, TypedProps, dsl::Behaviors},
};
use fraktor_utils_core_rs::core::sync::{SharedLock, SpinSyncMutex};

#[derive(Clone, Copy)]
enum Command {
  Greet,
}

fn greeter(greetings: SharedLock<Vec<&'static str>>) -> Behavior<Command> {
  Behaviors::receive_message(move |_ctx, message: &Command| {
    match message {
      | Command::Greet => greetings.with_lock(|greetings| greetings.push("hello")),
    }
    Ok(Behaviors::same())
  })
}

fn main() {
  let greetings = SharedLock::new_with_driver::<SpinSyncMutex<_>>(Vec::new());
  let props = TypedProps::from_behavior_factory({
    let greetings = greetings.clone();
    move || greeter(greetings.clone())
  });
  let system =
    TypedActorSystem::create_with_config(&props, ActorSystemConfig::new(StdTickDriver::default())).expect("system");
  let termination = system.when_terminated();
  let mut guardian = system.user_guardian_ref();

  guardian.tell(Command::Greet);
  wait_until(|| greetings.with_lock(|greetings| greetings.as_slice() == ["hello"]));

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
