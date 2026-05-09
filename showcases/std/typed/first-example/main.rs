use core::time::Duration;
use std::{thread, time::Instant};

use fraktor_actor_adaptor_std_rs::std::{StdBlocker, tick_driver::StdTickDriver};
use fraktor_actor_core_kernel_rs::actor::setup::ActorSystemConfig;
use fraktor_actor_core_typed_rs::{Behavior, TypedActorSystem, dsl::Behaviors};
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
  let system = TypedActorSystem::create_from_behavior_factory(
    {
      let greetings = greetings.clone();
      move || greeter(greetings.clone())
    },
    ActorSystemConfig::new(StdTickDriver::default()),
  )
  .expect("system");
  let termination = system.when_terminated();
  let mut guardian = system.user_guardian_ref();

  guardian.tell(Command::Greet);
  wait_until(|| greetings.with_lock(|greetings| greetings.as_slice() == ["hello"]), Duration::from_secs(10));
  println!("typed_first_example recorded greetings: {:?}", greetings.with_lock(|greetings| greetings.clone()));

  system.terminate().expect("terminate");
  termination.wait_blocking(&StdBlocker::new());
}

fn wait_until(mut condition: impl FnMut() -> bool, timeout: Duration) {
  let started = Instant::now();
  let deadline = started + timeout;
  let mut attempts = 0_u64;
  while Instant::now() < deadline {
    if condition() {
      return;
    }
    attempts += 1;
    thread::sleep(Duration::from_millis(1));
  }
  if condition() {
    return;
  }
  panic!("wait_until timed out after {:?} (attempts: {attempts})", started.elapsed());
}
