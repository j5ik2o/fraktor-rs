#![cfg(feature = "std")]

extern crate alloc;

use alloc::vec::Vec;

use cellactor_actor_core_rs::{
  Actor, ActorContext, ActorError, ActorErrorReason, ActorSystem, AnyMessage, AnyMessageView, ChildRef, NoStdToolbox,
  Props,
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
      let child_props = Props::<NoStdToolbox>::from_fn({
        let log = self.log.clone();
        move || RestartChild::new(log.clone())
      });
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
      let child_props = Props::from_fn(|| FatalChild);
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
