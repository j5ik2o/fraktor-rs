use core::time::Duration;
use std::thread;

use fraktor_actor_adaptor_std_rs::{StdBlocker, tick_driver::StdTickDriver};
use fraktor_actor_core_kernel_rs::{
  actor::{
    Actor, ActorContext,
    error::ActorError,
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
    setup::ActorSystemConfig,
  },
  dispatch::dispatcher::DEFAULT_BLOCKING_DISPATCHER_ID,
  system::ActorSystem,
};
use fraktor_utils_core_rs::sync::{SharedLock, SpinSyncMutex};

struct RunBlockingWork;

struct WorkerActor {
  events: SharedLock<Vec<&'static str>>,
}

impl Actor for WorkerActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<RunBlockingWork>().is_some() {
      self.events.with_lock(|events| events.push("blocking-dispatcher-work"));
    }
    Ok(())
  }
}

fn main() {
  let events = SharedLock::new_with_driver::<SpinSyncMutex<_>>(Vec::new());
  let props = Props::from_fn({
    let events = events.clone();
    move || WorkerActor { events: events.clone() }
  })
  .with_dispatcher_id(DEFAULT_BLOCKING_DISPATCHER_ID);
  let system =
    ActorSystem::create_from_props(&props, ActorSystemConfig::new(StdTickDriver::default())).expect("system");
  let termination = system.when_terminated();

  system.user_guardian_ref().tell(AnyMessage::new(RunBlockingWork));
  wait_until(|| events.with_lock(|events| events.as_slice() == ["blocking-dispatcher-work"]));
  println!("kernel_dispatchers recorded blocking dispatcher work");

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
