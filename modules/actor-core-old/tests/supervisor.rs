#![cfg(feature = "std")]

extern crate alloc;

use alloc::vec::Vec;
use core::time::Duration;

use cellactor_actor_core_rs::{
  Actor, ActorContext, ActorError, ActorErrorReason, ActorSystem, AnyMessage, AnyMessageView, ChildRef, Props,
  SupervisorDirective, SupervisorOptions, SupervisorStrategy, SupervisorStrategyKind,
};
use cellactor_utils_core_rs::sync::{ArcShared, sync_mutex_like::SpinSyncMutex};

struct Start;
struct TriggerRecoverable;
struct TriggerFatal;

#[test]
fn recoverable_failure_triggers_restart() {
  let child_log: ArcShared<SpinSyncMutex<Vec<&'static str>>> = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let child_slot: ArcShared<SpinSyncMutex<Option<ChildRef>>> = ArcShared::new(SpinSyncMutex::new(None));

  let props = Props::from_fn({
    let child_log = child_log.clone();
    let child_slot = child_slot.clone();
    move || RestartGuardian::new(child_log.clone(), child_slot.clone())
  });

  let system = ActorSystem::new(&props).expect("system");
  system.user_guardian_ref().tell(AnyMessage::new(Start)).expect("start guardian");

  let child = child_slot.lock().clone().expect("child ref");
  assert_eq!(child_log.lock().clone(), vec!["child_pre_start"]);

  child.tell(AnyMessage::new(TriggerRecoverable)).expect("trigger failure");

  assert_eq!(child_log.lock().clone(), vec!["child_pre_start", "child_fail", "child_post_stop", "child_pre_start"]);
}

#[test]
fn fatal_failure_stops_child() {
  let child_slot: ArcShared<SpinSyncMutex<Option<ChildRef>>> = ArcShared::new(SpinSyncMutex::new(None));

  let props = Props::from_fn({
    let child_slot = child_slot.clone();
    move || FatalGuardian::new(child_slot.clone())
  });

  let system = ActorSystem::new(&props).expect("system");
  system.user_guardian_ref().tell(AnyMessage::new(Start)).expect("start guardian");

  let child = child_slot.lock().clone().expect("child ref");
  child.tell(AnyMessage::new(TriggerFatal)).expect("trigger fatal");

  assert!(system.actor_ref(child.pid()).is_none(), "child should be removed after fatal failure");
}

#[test]
fn escalate_restarts_parent_via_root_supervisor() {
  let supervisor_log: ArcShared<SpinSyncMutex<Vec<&'static str>>> = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let child_log: ArcShared<SpinSyncMutex<Vec<&'static str>>> = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let supervisor_slot: ArcShared<SpinSyncMutex<Option<ChildRef>>> = ArcShared::new(SpinSyncMutex::new(None));
  let child_slot: ArcShared<SpinSyncMutex<Option<ChildRef>>> = ArcShared::new(SpinSyncMutex::new(None));

  let props = Props::from_fn({
    let supervisor_slot = supervisor_slot.clone();
    let child_slot = child_slot.clone();
    let supervisor_log = supervisor_log.clone();
    let child_log = child_log.clone();
    move || RootGuardian::new(supervisor_slot.clone(), child_slot.clone(), supervisor_log.clone(), child_log.clone())
  });

  let system = ActorSystem::new(&props).expect("system");
  system.user_guardian_ref().tell(AnyMessage::new(Start)).expect("start guardian");

  let supervisor = supervisor_slot.lock().clone().expect("supervisor ref");
  let child = child_slot.lock().clone().expect("child ref");

  assert_eq!(supervisor_log.lock().clone(), vec!["supervisor_pre_start"]);
  assert_eq!(child_log.lock().clone(), vec!["child_pre_start"]);

  child.tell(AnyMessage::new(TriggerRecoverable)).expect("trigger recoverable failure");

  let expected_supervisor_log = vec!["supervisor_pre_start", "supervisor_post_stop", "supervisor_pre_start"];
  assert_eq!(supervisor_log.lock().clone(), expected_supervisor_log);

  let child_log_entries = child_log.lock().clone();
  assert_eq!(child_log_entries.iter().filter(|entry| **entry == "child_pre_start").count(), 2);
  assert!(child_log_entries.contains(&"child_fail"));
  assert!(child_log_entries.contains(&"child_post_stop"));

  assert!(system.actor_ref(supervisor.pid()).is_some(), "supervisor should be restarted by root supervisor");
}

struct RestartGuardian {
  child_log:  ArcShared<SpinSyncMutex<Vec<&'static str>>>,
  child_slot: ArcShared<SpinSyncMutex<Option<ChildRef>>>,
}

impl RestartGuardian {
  fn new(
    child_log: ArcShared<SpinSyncMutex<Vec<&'static str>>>,
    child_slot: ArcShared<SpinSyncMutex<Option<ChildRef>>>,
  ) -> Self {
    Self { child_log, child_slot }
  }
}

impl Actor for RestartGuardian {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_some() && self.child_slot.lock().is_none() {
      let props = Props::from_fn({
        let child_log = self.child_log.clone();
        move || RestartChild::new(child_log.clone())
      });
      let child = ctx.spawn_child(&props).map_err(|_| ActorError::recoverable("spawn failed"))?;
      self.child_slot.lock().replace(child);
    }
    Ok(())
  }
}

struct RestartChild {
  log: ArcShared<SpinSyncMutex<Vec<&'static str>>>,
}

impl RestartChild {
  fn new(log: ArcShared<SpinSyncMutex<Vec<&'static str>>>) -> Self {
    Self { log }
  }
}

impl Actor for RestartChild {
  fn pre_start(&mut self, _ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    self.log.lock().push("child_pre_start");
    Ok(())
  }

  fn receive(&mut self, _ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<TriggerRecoverable>().is_some() {
      self.log.lock().push("child_fail");
      return Err(ActorError::recoverable("recoverable failure"));
    }
    Ok(())
  }

  fn post_stop(&mut self, _ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    self.log.lock().push("child_post_stop");
    Ok(())
  }
}

struct FatalGuardian {
  child_slot: ArcShared<SpinSyncMutex<Option<ChildRef>>>,
}

impl FatalGuardian {
  fn new(child_slot: ArcShared<SpinSyncMutex<Option<ChildRef>>>) -> Self {
    Self { child_slot }
  }
}

impl Actor for FatalGuardian {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_some() && self.child_slot.lock().is_none() {
      let props = Props::from_fn(|| FatalChild);
      let child = ctx.spawn_child(&props).map_err(|_| ActorError::recoverable("spawn failed"))?;
      self.child_slot.lock().replace(child);
    }
    Ok(())
  }
}

struct FatalChild;

impl Actor for FatalChild {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<TriggerFatal>().is_some() {
      return Err(ActorError::fatal(ActorErrorReason::new("fatal failure")));
    }
    Ok(())
  }
}

struct RootGuardian {
  supervisor_slot: ArcShared<SpinSyncMutex<Option<ChildRef>>>,
  child_slot:      ArcShared<SpinSyncMutex<Option<ChildRef>>>,
  supervisor_log:  ArcShared<SpinSyncMutex<Vec<&'static str>>>,
  child_log:       ArcShared<SpinSyncMutex<Vec<&'static str>>>,
}

impl RootGuardian {
  fn new(
    supervisor_slot: ArcShared<SpinSyncMutex<Option<ChildRef>>>,
    child_slot: ArcShared<SpinSyncMutex<Option<ChildRef>>>,
    supervisor_log: ArcShared<SpinSyncMutex<Vec<&'static str>>>,
    child_log: ArcShared<SpinSyncMutex<Vec<&'static str>>>,
  ) -> Self {
    Self { supervisor_slot, child_slot, supervisor_log, child_log }
  }
}

impl Actor for RootGuardian {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_some() && self.supervisor_slot.lock().is_none() {
      let supervisor_props = Props::from_fn({
        let supervisor_log = self.supervisor_log.clone();
        let child_slot = self.child_slot.clone();
        let child_log = self.child_log.clone();
        move || EscalatingSupervisor::new(supervisor_log.clone(), child_slot.clone(), child_log.clone())
      })
      .with_supervisor(SupervisorOptions::new(SupervisorStrategy::new(
        SupervisorStrategyKind::OneForOne,
        5,
        Duration::from_millis(10),
        escalate_decider,
      )));
      let supervisor = ctx.spawn_child(&supervisor_props).map_err(|_| ActorError::recoverable("spawn failed"))?;
      self.supervisor_slot.lock().replace(supervisor);
    }
    Ok(())
  }
}

struct EscalatingSupervisor {
  supervisor_log: ArcShared<SpinSyncMutex<Vec<&'static str>>>,
  child_slot:     ArcShared<SpinSyncMutex<Option<ChildRef>>>,
  child_log:      ArcShared<SpinSyncMutex<Vec<&'static str>>>,
}

impl EscalatingSupervisor {
  fn new(
    supervisor_log: ArcShared<SpinSyncMutex<Vec<&'static str>>>,
    child_slot: ArcShared<SpinSyncMutex<Option<ChildRef>>>,
    child_log: ArcShared<SpinSyncMutex<Vec<&'static str>>>,
  ) -> Self {
    Self { supervisor_log, child_slot, child_log }
  }
}

impl Actor for EscalatingSupervisor {
  fn pre_start(&mut self, ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    self.supervisor_log.lock().push("supervisor_pre_start");
    let child_props = Props::from_fn({
      let child_log = self.child_log.clone();
      move || EscalatingChild::new(child_log.clone())
    });
    let child = ctx.spawn_child(&child_props).map_err(|_| ActorError::recoverable("spawn child failed"))?;
    self.child_slot.lock().replace(child);
    Ok(())
  }

  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }

  fn post_stop(&mut self, _ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    self.supervisor_log.lock().push("supervisor_post_stop");
    Ok(())
  }
}

struct EscalatingChild {
  log: ArcShared<SpinSyncMutex<Vec<&'static str>>>,
}

impl EscalatingChild {
  fn new(log: ArcShared<SpinSyncMutex<Vec<&'static str>>>) -> Self {
    Self { log }
  }
}

impl Actor for EscalatingChild {
  fn pre_start(&mut self, _ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    self.log.lock().push("child_pre_start");
    Ok(())
  }

  fn receive(&mut self, _ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<TriggerRecoverable>().is_some() {
      self.log.lock().push("child_fail");
      return Err(ActorError::recoverable("recoverable failure"));
    }
    Ok(())
  }

  fn post_stop(&mut self, _ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    self.log.lock().push("child_post_stop");
    Ok(())
  }
}

fn escalate_decider(error: &ActorError) -> SupervisorDirective {
  match error {
    | ActorError::Recoverable(_) => SupervisorDirective::Escalate,
    | ActorError::Fatal(_) => SupervisorDirective::Stop,
  }
}
