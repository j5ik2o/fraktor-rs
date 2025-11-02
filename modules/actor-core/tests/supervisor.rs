#![cfg(not(target_os = "none"))]

extern crate alloc;

use alloc::vec::Vec;
use std::{
  panic::{AssertUnwindSafe, catch_unwind},
  thread,
  time::Duration,
};

use cellactor_actor_core_rs::{
  Actor, ActorContext, ActorError, ActorErrorReason, ActorSystem, AnyMessage, AnyMessageView, ChildRef, NoStdToolbox,
  Props, SupervisorDirective, SupervisorOptions, SupervisorStrategy, SupervisorStrategyKind,
};
use cellactor_utils_core_rs::sync::{ArcShared, NoStdMutex};

struct Start;
struct TriggerRecoverable;
struct TriggerFatal;

#[test]
fn recoverable_failure_restarts_child() {
  let log = ArcShared::new(NoStdMutex::new(Vec::new()));
  let child_slot = ArcShared::new(NoStdMutex::new(None));

  let props = Props::<NoStdToolbox>::from_fn({
    let log = log.clone();
    let child_slot = child_slot.clone();
    move || RestartGuardian::new(log.clone(), child_slot.clone())
  });

  let system = ActorSystem::new(&props).expect("system");
  system.user_guardian_ref().tell(AnyMessage::new(Start)).expect("start");

  let child = child_slot.lock().clone().expect("child");
  assert_eq!(*log.lock(), vec!["child_pre_start"]);

  child.tell(AnyMessage::new(TriggerRecoverable)).expect("recoverable");

  assert_eq!(*log.lock(), vec!["child_pre_start", "child_fail", "child_post_stop", "child_pre_start"],);
}

#[test]
fn fatal_failure_stops_child() {
  let child_slot = ArcShared::new(NoStdMutex::new(None));

  let props = Props::<NoStdToolbox>::from_fn({
    let child_slot = child_slot.clone();
    move || FatalGuardian::new(child_slot.clone())
  });

  let system = ActorSystem::new(&props).expect("system");
  system.user_guardian_ref().tell(AnyMessage::new(Start)).expect("start");

  let child = child_slot.lock().clone().expect("child");
  child.tell(AnyMessage::new(TriggerFatal)).expect("fatal");

  assert!(system.actor_ref(child.pid()).is_none());
}

#[test]
fn escalate_failure_restarts_supervisor() {
  let supervisor_log = ArcShared::new(NoStdMutex::new(Vec::new()));
  let child_log = ArcShared::new(NoStdMutex::new(Vec::new()));
  let supervisor_slot = ArcShared::new(NoStdMutex::new(None));
  let child_slot = ArcShared::new(NoStdMutex::new(None));

  let props = Props::<NoStdToolbox>::from_fn({
    let supervisor_slot = supervisor_slot.clone();
    let child_slot = child_slot.clone();
    let supervisor_log = supervisor_log.clone();
    let child_log = child_log.clone();
    move || RootGuardian::new(supervisor_slot.clone(), child_slot.clone(), supervisor_log.clone(), child_log.clone())
  });

  let system = ActorSystem::new(&props).expect("system");
  system.user_guardian_ref().tell(AnyMessage::new(Start)).expect("start");

  wait_until(|| child_slot.lock().is_some(), Duration::from_millis(20));

  let supervisor = supervisor_slot.lock().clone().expect("supervisor");
  let child = child_slot.lock().clone().expect("child");

  assert_eq!(*supervisor_log.lock(), vec!["supervisor_pre_start"]);
  assert_eq!(*child_log.lock(), vec!["child_pre_start"]);

  child.tell(AnyMessage::new(TriggerRecoverable)).expect("recoverable");

  wait_until(|| supervisor_log.lock().len() >= 3 && child_log.lock().len() >= 4, Duration::from_millis(20));

  let expected_supervisor_log = vec!["supervisor_pre_start", "supervisor_post_stop", "supervisor_pre_start"];
  assert_eq!(*supervisor_log.lock(), expected_supervisor_log);

  let child_entries = child_log.lock().clone();
  assert!(child_entries.contains(&"child_fail"));
  assert!(child_entries.contains(&"child_post_stop"));
  assert!(child_entries.iter().filter(|entry| **entry == "child_pre_start").count() >= 2);

  assert!(system.actor_ref(supervisor.pid()).is_some());
}

#[test]
fn panic_propagates_without_intervention() {
  let child_slot = ArcShared::new(NoStdMutex::new(None));
  let props = Props::<NoStdToolbox>::from_fn({
    let child_slot = child_slot.clone();
    move || PanicGuardian::new(child_slot.clone())
  });

  let system = ActorSystem::new(&props).expect("system");
  system.user_guardian_ref().tell(AnyMessage::new(Start)).expect("start");
  wait_until(|| child_slot.lock().is_some(), Duration::from_millis(20));
  let child = child_slot.lock().clone().expect("child");

  let result = catch_unwind(AssertUnwindSafe(|| {
    let _ = child.tell(AnyMessage::new("boom"));
  }));

  assert!(result.is_err());
}

fn wait_until(condition: impl Fn() -> bool, timeout: Duration) {
  let deadline = std::time::Instant::now() + timeout;
  while !condition() && std::time::Instant::now() < deadline {
    thread::yield_now();
  }
}

struct RestartGuardian {
  log:        ArcShared<NoStdMutex<Vec<&'static str>>>,
  child_slot: ArcShared<NoStdMutex<Option<ChildRef<NoStdToolbox>>>>,
}

impl RestartGuardian {
  fn new(
    log: ArcShared<NoStdMutex<Vec<&'static str>>>,
    child_slot: ArcShared<NoStdMutex<Option<ChildRef<NoStdToolbox>>>>,
  ) -> Self {
    Self { log, child_slot }
  }
}

impl Actor<NoStdToolbox> for RestartGuardian {
  fn receive(
    &mut self,
    ctx: &mut ActorContext<'_, NoStdToolbox>,
    message: AnyMessageView<'_, NoStdToolbox>,
  ) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_some() && self.child_slot.lock().is_none() {
      let log = self.log.clone();
      let child_props = Props::<NoStdToolbox>::from_fn(move || RestartChild::new(log.clone()));
      let child = ctx.spawn_child(&child_props).map_err(|_| ActorError::recoverable("spawn failed"))?;
      self.child_slot.lock().replace(child);
    }
    Ok(())
  }
}

struct RestartChild {
  log: ArcShared<NoStdMutex<Vec<&'static str>>>,
}

impl RestartChild {
  fn new(log: ArcShared<NoStdMutex<Vec<&'static str>>>) -> Self {
    Self { log }
  }
}

impl Actor<NoStdToolbox> for RestartChild {
  fn pre_start(&mut self, _ctx: &mut ActorContext<'_, NoStdToolbox>) -> Result<(), ActorError> {
    self.log.lock().push("child_pre_start");
    Ok(())
  }

  fn receive(
    &mut self,
    _ctx: &mut ActorContext<'_, NoStdToolbox>,
    message: AnyMessageView<'_, NoStdToolbox>,
  ) -> Result<(), ActorError> {
    if message.downcast_ref::<TriggerRecoverable>().is_some() {
      self.log.lock().push("child_fail");
      return Err(ActorError::recoverable("recoverable error"));
    }
    Ok(())
  }

  fn post_stop(&mut self, _ctx: &mut ActorContext<'_, NoStdToolbox>) -> Result<(), ActorError> {
    self.log.lock().push("child_post_stop");
    Ok(())
  }
}

struct FatalGuardian {
  child_slot: ArcShared<NoStdMutex<Option<ChildRef<NoStdToolbox>>>>,
}

impl FatalGuardian {
  fn new(child_slot: ArcShared<NoStdMutex<Option<ChildRef<NoStdToolbox>>>>) -> Self {
    Self { child_slot }
  }
}

impl Actor<NoStdToolbox> for FatalGuardian {
  fn receive(
    &mut self,
    ctx: &mut ActorContext<'_, NoStdToolbox>,
    message: AnyMessageView<'_, NoStdToolbox>,
  ) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_some() && self.child_slot.lock().is_none() {
      let child_props = Props::<NoStdToolbox>::from_fn(|| FatalChild);
      let child = ctx.spawn_child(&child_props).map_err(|_| ActorError::recoverable("spawn failed"))?;
      self.child_slot.lock().replace(child);
    }
    Ok(())
  }
}

struct FatalChild;

impl Actor<NoStdToolbox> for FatalChild {
  fn receive(
    &mut self,
    _ctx: &mut ActorContext<'_, NoStdToolbox>,
    message: AnyMessageView<'_, NoStdToolbox>,
  ) -> Result<(), ActorError> {
    if message.downcast_ref::<TriggerFatal>().is_some() {
      return Err(ActorError::fatal(ActorErrorReason::new("fatal failure")));
    }
    Ok(())
  }
}

struct RootGuardian {
  supervisor_slot: ArcShared<NoStdMutex<Option<ChildRef<NoStdToolbox>>>>,
  child_slot:      ArcShared<NoStdMutex<Option<ChildRef<NoStdToolbox>>>>,
  supervisor_log:  ArcShared<NoStdMutex<Vec<&'static str>>>,
  child_log:       ArcShared<NoStdMutex<Vec<&'static str>>>,
}

impl RootGuardian {
  fn new(
    supervisor_slot: ArcShared<NoStdMutex<Option<ChildRef<NoStdToolbox>>>>,
    child_slot: ArcShared<NoStdMutex<Option<ChildRef<NoStdToolbox>>>>,
    supervisor_log: ArcShared<NoStdMutex<Vec<&'static str>>>,
    child_log: ArcShared<NoStdMutex<Vec<&'static str>>>,
  ) -> Self {
    Self { supervisor_slot, child_slot, supervisor_log, child_log }
  }
}

impl Actor<NoStdToolbox> for RootGuardian {
  fn receive(
    &mut self,
    ctx: &mut ActorContext<'_, NoStdToolbox>,
    message: AnyMessageView<'_, NoStdToolbox>,
  ) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_some() && self.supervisor_slot.lock().is_none() {
      let supervisor_props = Props::<NoStdToolbox>::from_fn({
        let supervisor_log = self.supervisor_log.clone();
        let child_slot = self.child_slot.clone();
        let child_log = self.child_log.clone();
        move || SupervisorActor::new(supervisor_log.clone(), child_slot.clone(), child_log.clone())
      })
      .with_supervisor(SupervisorOptions::new(SupervisorStrategy::new(
        SupervisorStrategyKind::OneForOne,
        3,
        Duration::from_secs(1),
        |error| match error {
          | ActorError::Recoverable(_) => SupervisorDirective::Escalate,
          | ActorError::Fatal(_) => SupervisorDirective::Stop,
        },
      )));

      let supervisor = ctx.spawn_child(&supervisor_props).map_err(|_| ActorError::recoverable("spawn supervisor"))?;
      self.supervisor_slot.lock().replace(supervisor.clone());
      supervisor.tell(AnyMessage::new(Start)).map_err(|_| ActorError::recoverable("start supervisor"))?;
    }
    Ok(())
  }
}

struct SupervisorActor {
  log:        ArcShared<NoStdMutex<Vec<&'static str>>>,
  child_slot: ArcShared<NoStdMutex<Option<ChildRef<NoStdToolbox>>>>,
  child_log:  ArcShared<NoStdMutex<Vec<&'static str>>>,
}

impl SupervisorActor {
  fn new(
    log: ArcShared<NoStdMutex<Vec<&'static str>>>,
    child_slot: ArcShared<NoStdMutex<Option<ChildRef<NoStdToolbox>>>>,
    child_log: ArcShared<NoStdMutex<Vec<&'static str>>>,
  ) -> Self {
    Self { log, child_slot, child_log }
  }
}

impl Actor<NoStdToolbox> for SupervisorActor {
  fn pre_start(&mut self, ctx: &mut ActorContext<'_, NoStdToolbox>) -> Result<(), ActorError> {
    self.log.lock().push("supervisor_pre_start");
    let child_log = self.child_log.clone();
    let child_props = Props::<NoStdToolbox>::from_fn(move || RestartChild::new(child_log.clone()));
    let child = ctx.spawn_child(&child_props).map_err(|_| ActorError::recoverable("spawn child"))?;
    self.child_slot.lock().replace(child);
    Ok(())
  }

  fn post_stop(&mut self, _ctx: &mut ActorContext<'_, NoStdToolbox>) -> Result<(), ActorError> {
    self.log.lock().push("supervisor_post_stop");
    Ok(())
  }

  fn receive(
    &mut self,
    ctx: &mut ActorContext<'_, NoStdToolbox>,
    message: AnyMessageView<'_, NoStdToolbox>,
  ) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_some() && self.child_slot.lock().is_none() {
      let child_log = self.child_log.clone();
      let child_props = Props::<NoStdToolbox>::from_fn(move || RestartChild::new(child_log.clone()));
      let child = ctx.spawn_child(&child_props).map_err(|_| ActorError::recoverable("spawn child"))?;
      self.child_slot.lock().replace(child);
    }
    Ok(())
  }
}

struct PanicGuardian {
  child_slot: ArcShared<NoStdMutex<Option<ChildRef<NoStdToolbox>>>>,
}

impl PanicGuardian {
  fn new(child_slot: ArcShared<NoStdMutex<Option<ChildRef<NoStdToolbox>>>>) -> Self {
    Self { child_slot }
  }
}

impl Actor<NoStdToolbox> for PanicGuardian {
  fn receive(
    &mut self,
    ctx: &mut ActorContext<'_, NoStdToolbox>,
    message: AnyMessageView<'_, NoStdToolbox>,
  ) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_some() && self.child_slot.lock().is_none() {
      let child_props = Props::<NoStdToolbox>::from_fn(|| PanicChild);
      let child = ctx.spawn_child(&child_props).map_err(|_| ActorError::recoverable("spawn failed"))?;
      self.child_slot.lock().replace(child);
    }
    Ok(())
  }
}

struct PanicChild;

impl Actor<NoStdToolbox> for PanicChild {
  fn receive(
    &mut self,
    _ctx: &mut ActorContext<'_, NoStdToolbox>,
    _message: AnyMessageView<'_, NoStdToolbox>,
  ) -> Result<(), ActorError> {
    panic!("child panic");
  }
}
