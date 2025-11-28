#![cfg(not(target_os = "none"))]

extern crate alloc;

use alloc::vec::Vec;
use std::{
  panic::{AssertUnwindSafe, catch_unwind},
  thread,
  time::Duration,
};

use fraktor_actor_rs::core::{
  actor_prim::{Actor, ActorContextGeneric, ChildRef},
  error::{ActorError, ActorErrorReason},
  event_stream::{EventStreamEvent, EventStreamSubscriber, subscriber_handle},
  lifecycle::LifecycleStage,
  messaging::{AnyMessage, AnyMessageViewGeneric},
  props::Props,
  supervision::{SupervisorDirective, SupervisorStrategy, SupervisorStrategyKind},
  system::ActorSystem,
};
use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdMutex, NoStdToolbox},
  sync::ArcShared,
};

struct Start;
struct TriggerRecoverable;
struct TriggerFatal;

struct RecordingSubscriber {
  events: ArcShared<NoStdMutex<Vec<EventStreamEvent<NoStdToolbox>>>>,
}

impl EventStreamSubscriber<NoStdToolbox> for RecordingSubscriber {
  fn on_event(&mut self, event: &EventStreamEvent<NoStdToolbox>) {
    self.events.lock().push(event.clone());
  }
}

#[test]
fn recoverable_failure_restarts_child() {
  let log = ArcShared::new(NoStdMutex::new(Vec::new()));
  let child_slot = ArcShared::new(NoStdMutex::new(None));

  let props = Props::from_fn({
    let log = log.clone();
    let child_slot = child_slot.clone();
    move || RestartGuardian::new(log.clone(), child_slot.clone())
  });

  let tick_driver = fraktor_actor_rs::core::scheduler::TickDriverConfig::manual(
    fraktor_actor_rs::core::scheduler::ManualTestDriver::new(),
  );
  let system = ActorSystem::new(&props, tick_driver).expect("system");
  system.user_guardian_ref().tell(AnyMessage::new(Start)).expect("start");

  let child = child_slot.lock().clone().expect("child");
  assert_eq!(*log.lock(), vec!["child_pre_start"]);

  child.tell(AnyMessage::new(TriggerRecoverable)).expect("recoverable");

  assert_eq!(*log.lock(), vec!["child_pre_start", "child_fail", "child_post_stop", "child_pre_start"],);
}

#[test]
fn fatal_failure_stops_child() {
  let child_slot = ArcShared::new(NoStdMutex::new(None));

  let props = Props::from_fn({
    let child_slot = child_slot.clone();
    move || FatalGuardian::new(child_slot.clone())
  });

  let tick_driver = fraktor_actor_rs::core::scheduler::TickDriverConfig::manual(
    fraktor_actor_rs::core::scheduler::ManualTestDriver::new(),
  );
  let system = ActorSystem::new(&props, tick_driver).expect("system");

  let events = ArcShared::new(NoStdMutex::new(Vec::new()));
  let subscriber = subscriber_handle(RecordingSubscriber { events: events.clone() });
  let _subscription = system.subscribe_event_stream(&subscriber);

  system.user_guardian_ref().tell(AnyMessage::new(Start)).expect("start");

  let child = child_slot.lock().clone().expect("child");
  let child_pid = child.pid();
  child.tell(AnyMessage::new(TriggerFatal)).expect("fatal");

  wait_until(
    || {
      events.lock().iter().any(|event| {
        matches!(event, EventStreamEvent::Lifecycle(lifecycle)
        if lifecycle.stage() == LifecycleStage::Stopped && lifecycle.pid() == child_pid)
      })
    },
    Duration::from_millis(100),
  );
}

#[test]
fn escalate_failure_restarts_supervisor() {
  let supervisor_log = ArcShared::new(NoStdMutex::new(Vec::new()));
  let child_log = ArcShared::new(NoStdMutex::new(Vec::new()));
  let supervisor_slot = ArcShared::new(NoStdMutex::new(None));
  let child_slot = ArcShared::new(NoStdMutex::new(None));

  let props = Props::from_fn({
    let supervisor_slot = supervisor_slot.clone();
    let child_slot = child_slot.clone();
    let supervisor_log = supervisor_log.clone();
    let child_log = child_log.clone();
    move || RootGuardian::new(supervisor_slot.clone(), child_slot.clone(), supervisor_log.clone(), child_log.clone())
  });

  let tick_driver = fraktor_actor_rs::core::scheduler::TickDriverConfig::manual(
    fraktor_actor_rs::core::scheduler::ManualTestDriver::new(),
  );
  let system = ActorSystem::new(&props, tick_driver).expect("system");
  system.user_guardian_ref().tell(AnyMessage::new(Start)).expect("start");

  wait_until(|| child_slot.lock().is_some(), Duration::from_millis(20));

  let _supervisor = supervisor_slot.lock().clone().expect("supervisor");
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

  // supervisor が再起動して生存していることは、supervisor_log に2回目の "supervisor_pre_start" が
  // 記録されていることで確認できている
}

#[test]
fn panic_propagates_without_intervention() {
  let child_slot = ArcShared::new(NoStdMutex::new(None));
  let props = Props::from_fn({
    let child_slot = child_slot.clone();
    move || PanicGuardian::new(child_slot.clone())
  });

  let tick_driver = fraktor_actor_rs::core::scheduler::TickDriverConfig::manual(
    fraktor_actor_rs::core::scheduler::ManualTestDriver::new(),
  );
  let system = ActorSystem::new(&props, tick_driver).expect("system");
  system.user_guardian_ref().tell(AnyMessage::new(Start)).expect("start");
  wait_until(|| child_slot.lock().is_some(), Duration::from_millis(20));
  let child = child_slot.lock().clone().expect("child");

  let result = catch_unwind(AssertUnwindSafe(|| {
    let _ = child.tell(AnyMessage::new("boom"));
  }));

  assert!(result.is_err());
}

fn wait_until(condition: impl Fn() -> bool, timeout: Duration) {
  let dead_line = std::time::Instant::now() + timeout;
  while !condition() && std::time::Instant::now() < dead_line {
    thread::yield_now();
  }
}

struct RestartGuardian {
  log:        ArcShared<NoStdMutex<Vec<&'static str>>>,
  child_slot: ArcShared<NoStdMutex<Option<ChildRef>>>,
}

impl RestartGuardian {
  fn new(log: ArcShared<NoStdMutex<Vec<&'static str>>>, child_slot: ArcShared<NoStdMutex<Option<ChildRef>>>) -> Self {
    Self { log, child_slot }
  }
}

impl Actor for RestartGuardian {
  fn receive(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, NoStdToolbox>,
    message: AnyMessageViewGeneric<'_, NoStdToolbox>,
  ) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_some() && self.child_slot.lock().is_none() {
      let log = self.log.clone();
      let child_props = Props::from_fn(move || RestartChild::new(log.clone()));
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

impl Actor for RestartChild {
  fn pre_start(&mut self, _ctx: &mut ActorContextGeneric<'_, NoStdToolbox>) -> Result<(), ActorError> {
    self.log.lock().push("child_pre_start");
    Ok(())
  }

  fn receive(
    &mut self,
    _ctx: &mut ActorContextGeneric<'_, NoStdToolbox>,
    message: AnyMessageViewGeneric<'_, NoStdToolbox>,
  ) -> Result<(), ActorError> {
    if message.downcast_ref::<TriggerRecoverable>().is_some() {
      self.log.lock().push("child_fail");
      return Err(ActorError::recoverable("recoverable error"));
    }
    Ok(())
  }

  fn post_stop(&mut self, _ctx: &mut ActorContextGeneric<'_, NoStdToolbox>) -> Result<(), ActorError> {
    self.log.lock().push("child_post_stop");
    Ok(())
  }
}

struct FatalGuardian {
  child_slot: ArcShared<NoStdMutex<Option<ChildRef>>>,
}

impl FatalGuardian {
  fn new(child_slot: ArcShared<NoStdMutex<Option<ChildRef>>>) -> Self {
    Self { child_slot }
  }
}

impl Actor for FatalGuardian {
  fn receive(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, NoStdToolbox>,
    message: AnyMessageViewGeneric<'_, NoStdToolbox>,
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

impl Actor for FatalChild {
  fn receive(
    &mut self,
    _ctx: &mut ActorContextGeneric<'_, NoStdToolbox>,
    message: AnyMessageViewGeneric<'_, NoStdToolbox>,
  ) -> Result<(), ActorError> {
    if message.downcast_ref::<TriggerFatal>().is_some() {
      return Err(ActorError::fatal(ActorErrorReason::new("fatal failure")));
    }
    Ok(())
  }
}

struct RootGuardian {
  supervisor_slot: ArcShared<NoStdMutex<Option<ChildRef>>>,
  child_slot:      ArcShared<NoStdMutex<Option<ChildRef>>>,
  supervisor_log:  ArcShared<NoStdMutex<Vec<&'static str>>>,
  child_log:       ArcShared<NoStdMutex<Vec<&'static str>>>,
}

impl RootGuardian {
  fn new(
    supervisor_slot: ArcShared<NoStdMutex<Option<ChildRef>>>,
    child_slot: ArcShared<NoStdMutex<Option<ChildRef>>>,
    supervisor_log: ArcShared<NoStdMutex<Vec<&'static str>>>,
    child_log: ArcShared<NoStdMutex<Vec<&'static str>>>,
  ) -> Self {
    Self { supervisor_slot, child_slot, supervisor_log, child_log }
  }
}

impl Actor for RootGuardian {
  fn receive(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, NoStdToolbox>,
    message: AnyMessageViewGeneric<'_, NoStdToolbox>,
  ) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_some() && self.supervisor_slot.lock().is_none() {
      let supervisor_props = Props::from_fn({
        let supervisor_log = self.supervisor_log.clone();
        let child_slot = self.child_slot.clone();
        let child_log = self.child_log.clone();
        move || SupervisorActor::new(supervisor_log.clone(), child_slot.clone(), child_log.clone())
      });

      let supervisor = ctx.spawn_child(&supervisor_props).map_err(|_| ActorError::recoverable("spawn supervisor"))?;
      self.supervisor_slot.lock().replace(supervisor.clone());
      supervisor.tell(AnyMessage::new(Start)).map_err(|_| ActorError::recoverable("start supervisor"))?;
    }
    Ok(())
  }
}

struct SupervisorActor {
  log:        ArcShared<NoStdMutex<Vec<&'static str>>>,
  child_slot: ArcShared<NoStdMutex<Option<ChildRef>>>,
  child_log:  ArcShared<NoStdMutex<Vec<&'static str>>>,
}

impl SupervisorActor {
  fn new(
    log: ArcShared<NoStdMutex<Vec<&'static str>>>,
    child_slot: ArcShared<NoStdMutex<Option<ChildRef>>>,
    child_log: ArcShared<NoStdMutex<Vec<&'static str>>>,
  ) -> Self {
    Self { log, child_slot, child_log }
  }
}

impl Actor for SupervisorActor {
  fn pre_start(&mut self, ctx: &mut ActorContextGeneric<'_, NoStdToolbox>) -> Result<(), ActorError> {
    self.log.lock().push("supervisor_pre_start");
    let child_log = self.child_log.clone();
    let child_props = Props::from_fn(move || RestartChild::new(child_log.clone()));
    let child = ctx.spawn_child(&child_props).map_err(|_| ActorError::recoverable("spawn child"))?;
    self.child_slot.lock().replace(child);
    Ok(())
  }

  fn post_stop(&mut self, _ctx: &mut ActorContextGeneric<'_, NoStdToolbox>) -> Result<(), ActorError> {
    self.log.lock().push("supervisor_post_stop");
    Ok(())
  }

  fn receive(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, NoStdToolbox>,
    message: AnyMessageViewGeneric<'_, NoStdToolbox>,
  ) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_some() && self.child_slot.lock().is_none() {
      let child_log = self.child_log.clone();
      let child_props = Props::from_fn(move || RestartChild::new(child_log.clone()));
      let child = ctx.spawn_child(&child_props).map_err(|_| ActorError::recoverable("spawn child"))?;
      self.child_slot.lock().replace(child);
    }
    Ok(())
  }

  fn supervisor_strategy(&mut self, _ctx: &mut ActorContextGeneric<'_, NoStdToolbox>) -> SupervisorStrategy {
    SupervisorStrategy::new(SupervisorStrategyKind::OneForOne, 3, Duration::from_secs(1), |error| match error {
      | ActorError::Recoverable(_) => SupervisorDirective::Escalate,
      | ActorError::Fatal(_) => SupervisorDirective::Stop,
    })
  }
}

struct PanicGuardian {
  child_slot: ArcShared<NoStdMutex<Option<ChildRef>>>,
}

impl PanicGuardian {
  fn new(child_slot: ArcShared<NoStdMutex<Option<ChildRef>>>) -> Self {
    Self { child_slot }
  }
}

impl Actor for PanicGuardian {
  fn receive(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, NoStdToolbox>,
    message: AnyMessageViewGeneric<'_, NoStdToolbox>,
  ) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_some() && self.child_slot.lock().is_none() {
      let child_props = Props::from_fn(|| PanicChild);
      let child = ctx.spawn_child(&child_props).map_err(|_| ActorError::recoverable("spawn failed"))?;
      self.child_slot.lock().replace(child);
    }
    Ok(())
  }
}

struct PanicChild;

impl Actor for PanicChild {
  fn receive(
    &mut self,
    _ctx: &mut ActorContextGeneric<'_, NoStdToolbox>,
    _message: AnyMessageViewGeneric<'_, NoStdToolbox>,
  ) -> Result<(), ActorError> {
    panic!("child panic");
  }
}
