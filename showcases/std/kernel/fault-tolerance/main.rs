use core::time::Duration;
use std::thread;

use fraktor_actor_adaptor_std_rs::std::{StdBlocker, tick_driver::StdTickDriver};
use fraktor_actor_core_rs::core::kernel::{
  actor::{
    Actor, ActorContext,
    error::{ActorError, ActorErrorReason},
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
    setup::ActorSystemConfig,
  },
  system::ActorSystem,
};
use fraktor_utils_core_rs::core::sync::{SharedLock, SpinSyncMutex};

struct Start;
struct Crash;
struct Work;

struct GuardianActor {
  events: SharedLock<Vec<&'static str>>,
}

impl Actor for GuardianActor {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_none() {
      return Ok(());
    }

    let child_props = Props::from_fn({
      let events = self.events.clone();
      move || RestartingActor { events: events.clone() }
    });
    let mut child = ctx
      .spawn_child(&child_props)
      .map_err(|error| ActorError::recoverable(format!("spawn child failed: {error:?}")))?;
    child.try_tell(AnyMessage::new(Crash)).map_err(|error| ActorError::recoverable(format!("{error:?}")))?;
    child.try_tell(AnyMessage::new(Work)).map_err(|error| ActorError::recoverable(format!("{error:?}")))?;
    Ok(())
  }
}

struct RestartingActor {
  events: SharedLock<Vec<&'static str>>,
}

impl Actor for RestartingActor {
  fn pre_start(&mut self, _ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    self.events.with_lock(|events| events.push("pre-start"));
    Ok(())
  }

  fn receive(&mut self, _ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<Crash>().is_some() {
      return Err(ActorError::recoverable("fault-tolerance example crash"));
    }
    if message.downcast_ref::<Work>().is_some() {
      self.events.with_lock(|events| events.push("work-after-restart"));
    }
    Ok(())
  }

  fn pre_restart(&mut self, _ctx: &mut ActorContext<'_>, _reason: &ActorErrorReason) -> Result<(), ActorError> {
    self.events.with_lock(|events| events.push("pre-restart"));
    Ok(())
  }

  fn post_restart(&mut self, _ctx: &mut ActorContext<'_>, _reason: &ActorErrorReason) -> Result<(), ActorError> {
    self.events.with_lock(|events| events.push("post-restart"));
    Ok(())
  }
}

fn main() {
  let events = SharedLock::new_with_driver::<SpinSyncMutex<_>>(Vec::new());
  let props = Props::from_fn({
    let events = events.clone();
    move || GuardianActor { events: events.clone() }
  });
  let system =
    ActorSystem::create_from_props(&props, ActorSystemConfig::new(StdTickDriver::default())).expect("system");
  let termination = system.when_terminated();

  system.user_guardian_ref().tell(AnyMessage::new(Start));
  wait_until(|| events.with_lock(|events| events.contains(&"work-after-restart")));
  let snapshot = events.with_lock(|events| events.clone());
  assert!(snapshot.contains(&"pre-restart"));
  assert!(snapshot.contains(&"post-restart"));
  println!("kernel_fault_tolerance observed events: {snapshot:?}");

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
