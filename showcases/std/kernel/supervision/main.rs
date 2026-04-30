#![cfg(not(target_os = "none"))]

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
    supervision::{
      RestartLimit, SupervisorDirective, SupervisorStrategy, SupervisorStrategyConfig, SupervisorStrategyKind,
    },
  },
  system::ActorSystem,
};
use fraktor_utils_core_rs::core::sync::{SharedLock, SpinSyncMutex};

struct Start;
struct Crash;
struct Work;

struct ParentActor {
  events: SharedLock<Vec<&'static str>>,
}

impl Actor for ParentActor {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_none() {
      return Ok(());
    }

    let child_props = Props::from_fn({
      let events = self.events.clone();
      move || WorkerActor { events: events.clone() }
    });
    let mut child = ctx
      .spawn_child(&child_props)
      .map_err(|error| ActorError::recoverable(format!("spawn child failed: {error:?}")))?;
    child.try_tell(AnyMessage::new(Crash)).map_err(|error| ActorError::recoverable(format!("{error:?}")))?;
    child.try_tell(AnyMessage::new(Work)).map_err(|error| ActorError::recoverable(format!("{error:?}")))?;
    Ok(())
  }

  fn supervisor_strategy(&self, _ctx: &mut ActorContext<'_>) -> SupervisorStrategyConfig {
    SupervisorStrategy::new(
      SupervisorStrategyKind::OneForOne,
      RestartLimit::WithinWindow(3),
      Duration::from_secs(1),
      |error| match error {
        | ActorError::Recoverable(_) => SupervisorDirective::Restart,
        | ActorError::Fatal(_) => SupervisorDirective::Stop,
        | ActorError::Escalate(_) => SupervisorDirective::Escalate,
      },
    )
    .into()
  }
}

struct WorkerActor {
  events: SharedLock<Vec<&'static str>>,
}

impl Actor for WorkerActor {
  fn pre_start(&mut self, _ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    self.events.with_lock(|events| events.push("worker-started"));
    Ok(())
  }

  fn receive(&mut self, _ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<Crash>().is_some() {
      return Err(ActorError::recoverable("supervision example crash"));
    }
    if message.downcast_ref::<Work>().is_some() {
      self.events.with_lock(|events| events.push("work-accepted"));
    }
    Ok(())
  }
}

fn main() {
  let events = SharedLock::new_with_driver::<SpinSyncMutex<_>>(Vec::new());
  let props = Props::from_fn({
    let events = events.clone();
    move || ParentActor { events: events.clone() }
  });
  let system =
    ActorSystem::create_with_config(&props, ActorSystemConfig::new(StdTickDriver::default())).expect("system");
  let termination = system.when_terminated();

  system.user_guardian_ref().tell(AnyMessage::new(Start));
  wait_until(|| events.with_lock(|events| events.contains(&"work-accepted")));
  assert!(events.with_lock(|events| events.iter().filter(|event| **event == "worker-started").count() >= 2));

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
