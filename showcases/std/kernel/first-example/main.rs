use core::time::Duration;
use std::thread;

use fraktor_actor_adaptor_std_rs::std::{StdBlocker, tick_driver::StdTickDriver};
use fraktor_actor_core_rs::core::kernel::{
  actor::{
    Actor, ActorContext,
    error::ActorError,
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
    setup::ActorSystemConfig,
  },
  system::ActorSystem,
};
use fraktor_utils_core_rs::core::sync::{SharedLock, SpinSyncMutex};

struct Greet;

struct GreeterActor {
  greetings: SharedLock<Vec<&'static str>>,
}

impl Actor for GreeterActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<Greet>().is_some() {
      self.greetings.with_lock(|greetings| greetings.push("hello"));
    }
    Ok(())
  }
}

fn main() {
  let greetings = SharedLock::new_with_driver::<SpinSyncMutex<_>>(Vec::new());
  let props = Props::from_fn({
    let greetings = greetings.clone();
    move || GreeterActor { greetings: greetings.clone() }
  });
  let system =
    ActorSystem::create_from_props(&props, ActorSystemConfig::new(StdTickDriver::default())).expect("system");
  let termination = system.when_terminated();

  system.user_guardian_ref().tell(AnyMessage::new(Greet));
  wait_until(|| greetings.with_lock(|greetings| greetings.as_slice() == ["hello"]));
  let snapshot = greetings.with_lock(|greetings| greetings.clone());
  println!("kernel_first_example recorded greetings: {snapshot:?}");

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
