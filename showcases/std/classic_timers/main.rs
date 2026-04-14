#![cfg(not(target_os = "none"))]

use core::time::Duration;
use std::{thread, vec::Vec};

use fraktor_actor_adaptor_std_rs::std::tick_driver::StdTickDriver;
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

struct Start;
struct Tick;

struct TimerActor {
  events: SharedLock<Vec<&'static str>>,
}

impl TimerActor {
  fn new(events: SharedLock<Vec<&'static str>>) -> Self {
    Self { events }
  }
}

impl Actor for TimerActor {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_some() {
      ctx
        .timers()
        .start_single_timer("example-tick", AnyMessage::new(Tick), Duration::from_millis(1))
        .map_err(|_| ActorError::recoverable("timer schedule failed"))?;
      return Ok(());
    }
    if message.downcast_ref::<Tick>().is_some() {
      self.events.with_lock(|events| events.push("tick"));
    }
    Ok(())
  }
}

fn main() {
  let events = SharedLock::new_with_driver::<SpinSyncMutex<_>>(Vec::new());
  let props = Props::from_fn({
    let events = events.clone();
    move || TimerActor::new(events.clone())
  });
  let system =
    ActorSystem::create_with_config(&props, ActorSystemConfig::new(StdTickDriver::default())).expect("system");

  system.user_guardian_ref().tell(AnyMessage::new(Start));

  wait_until(|| events.with_lock(|events| !events.is_empty()));
  assert_eq!(events.with_lock(|events| events.clone()), vec!["tick"]);

  system.terminate().expect("terminate");
}

fn wait_until(mut condition: impl FnMut() -> bool) {
  for _ in 0..10_000 {
    if condition() {
      return;
    }
    thread::yield_now();
  }
  assert!(condition());
}
