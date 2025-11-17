#![cfg(not(target_os = "none"))]

extern crate alloc;

use alloc::vec::Vec;
use std::{thread, time::Duration};

use fraktor_actor_core_rs::{
  NoStdToolbox,
  actor_prim::{Actor, ActorContextGeneric},
  error::ActorError,
  messaging::{AnyMessage, AnyMessageViewGeneric},
  props::Props,
  system::ActorSystem,
};
use fraktor_utils_core_rs::sync::{ArcShared, NoStdMutex};

struct Start;

#[test]
fn terminate_signals_future() {
  let props = Props::from_fn(|| IdleGuardian);
  let system = ActorSystem::new(&props).expect("system");
  let termination = system.when_terminated();
  system.terminate().expect("terminate");
  system.run_until_terminated();
  assert!(termination.is_ready());
}

#[test]
fn stop_self_propagates_to_children() {
  let child_states = ArcShared::new(NoStdMutex::new(Vec::new()));
  let props = Props::from_fn({
    let child_states = child_states.clone();
    move || ParentGuardian::new(child_states.clone())
  });

  let system = ActorSystem::new(&props).expect("system");
  system.user_guardian_ref().tell(AnyMessage::new(Start)).expect("start");

  let dead_line = std::time::Instant::now() + Duration::from_millis(20);
  while child_states.lock().len() < 2 && std::time::Instant::now() < dead_line {
    thread::yield_now();
  }

  assert_eq!(child_states.lock().clone(), vec!["child_pre_start", "child_post_stop"]);
}

struct IdleGuardian;

impl Actor for IdleGuardian {
  fn receive(
    &mut self,
    _ctx: &mut ActorContextGeneric<'_, NoStdToolbox>,
    _message: AnyMessageViewGeneric<'_, NoStdToolbox>,
  ) -> Result<(), ActorError> {
    Ok(())
  }
}

struct ParentGuardian {
  child_states: ArcShared<NoStdMutex<Vec<&'static str>>>,
}

impl ParentGuardian {
  fn new(child_states: ArcShared<NoStdMutex<Vec<&'static str>>>) -> Self {
    Self { child_states }
  }
}

impl Actor for ParentGuardian {
  fn receive(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, NoStdToolbox>,
    message: AnyMessageViewGeneric<'_, NoStdToolbox>,
  ) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_some() {
      let states = self.child_states.clone();
      let child_props = Props::from_fn(move || RecordingChild::new(states.clone()));
      let _ = ctx.spawn_child(&child_props).map_err(|_| ActorError::recoverable("spawn failed"))?;
      ctx.stop_self().map_err(|_| ActorError::recoverable("stop failed"))?;
    }
    Ok(())
  }
}

struct RecordingChild {
  states: ArcShared<NoStdMutex<Vec<&'static str>>>,
}

impl RecordingChild {
  fn new(states: ArcShared<NoStdMutex<Vec<&'static str>>>) -> Self {
    Self { states }
  }
}

impl Actor for RecordingChild {
  fn pre_start(&mut self, _ctx: &mut ActorContextGeneric<'_, NoStdToolbox>) -> Result<(), ActorError> {
    self.states.lock().push("child_pre_start");
    Ok(())
  }

  fn receive(
    &mut self,
    _ctx: &mut ActorContextGeneric<'_, NoStdToolbox>,
    _message: AnyMessageViewGeneric<'_, NoStdToolbox>,
  ) -> Result<(), ActorError> {
    Ok(())
  }

  fn post_stop(&mut self, _ctx: &mut ActorContextGeneric<'_, NoStdToolbox>) -> Result<(), ActorError> {
    self.states.lock().push("child_post_stop");
    Ok(())
  }
}
