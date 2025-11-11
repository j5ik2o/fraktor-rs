#![cfg(not(target_os = "none"))]

use std::{
  thread,
  time::{Duration, Instant},
};

use fraktor_actor_core_rs::{
  NoStdToolbox,
  actor_prim::{Actor, ActorContextGeneric, ChildRef, Pid},
  error::ActorError,
  messaging::{AnyMessage, AnyMessageView},
  props::Props,
  system::ActorSystem,
};
use fraktor_utils_core_rs::sync::{ArcShared, NoStdMutex};

struct SpawnChild;
struct StopChild;
struct UnwatchChild;
struct WatchAfterStop;
struct QueueUserEvent;
struct SpawnSecondaryWatcherMessage {
  log: ArcShared<NoStdMutex<Vec<Pid>>>,
}
struct StartCycle;
struct UserProbe;

struct PassiveChild;

impl Actor for PassiveChild {
  fn receive(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, NoStdToolbox>,
    message: AnyMessageView<'_, NoStdToolbox>,
  ) -> Result<(), ActorError> {
    if message.downcast_ref::<StopChild>().is_some() {
      ctx.stop_self().ok();
    }
    Ok(())
  }
}

struct HarnessWatcher {
  terminated_log: ArcShared<NoStdMutex<Vec<Pid>>>,
  order_log:      ArcShared<NoStdMutex<Vec<&'static str>>>,
  child_slot:     ArcShared<NoStdMutex<Option<ChildRef>>>,
}

impl HarnessWatcher {
  fn new(
    terminated_log: ArcShared<NoStdMutex<Vec<Pid>>>,
    order_log: ArcShared<NoStdMutex<Vec<&'static str>>>,
    child_slot: ArcShared<NoStdMutex<Option<ChildRef>>>,
  ) -> Self {
    Self { terminated_log, order_log, child_slot }
  }

  fn spawn_child(&self, ctx: &mut ActorContextGeneric<'_, NoStdToolbox>) -> Result<(), ActorError> {
    if self.child_slot.lock().is_some() {
      return Ok(());
    }
    let props = Props::from_fn(|| PassiveChild);
    let child =
      ctx.spawn_child(&props).map_err(|error| ActorError::recoverable(format!("spawn failed: {:?}", error)))?;
    ctx.watch(child.actor_ref()).map_err(|_| ActorError::recoverable("watch failed"))?;
    self.child_slot.lock().replace(child);
    Ok(())
  }

  fn stop_child(&self) {
    if let Some(child) = self.child_slot.lock().as_ref() {
      let _ = child.stop();
    }
  }
}

impl Actor for HarnessWatcher {
  fn receive(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, NoStdToolbox>,
    message: AnyMessageView<'_, NoStdToolbox>,
  ) -> Result<(), ActorError> {
    if message.downcast_ref::<SpawnChild>().is_some() {
      return self.spawn_child(ctx);
    }
    if message.downcast_ref::<StopChild>().is_some() {
      self.stop_child();
      return Ok(());
    }
    if message.downcast_ref::<UnwatchChild>().is_some() {
      if let Some(child) = self.child_slot.lock().as_ref() {
        ctx.unwatch(child.actor_ref()).map_err(|_| ActorError::recoverable("unwatch failed"))?;
      }
      return Ok(());
    }
    if message.downcast_ref::<WatchAfterStop>().is_some() {
      if let Some(child) = self.child_slot.lock().as_ref() {
        ctx.watch(child.actor_ref()).map_err(|_| ActorError::recoverable("rewatch failed"))?;
      }
      return Ok(());
    }
    if message.downcast_ref::<QueueUserEvent>().is_some() {
      ctx.self_ref().tell(AnyMessage::new(UserProbe)).map_err(|_| ActorError::recoverable("self send failed"))?;
      self.stop_child();
      return Ok(());
    }
    if message.downcast_ref::<UserProbe>().is_some() {
      self.order_log.lock().push("user");
      return Ok(());
    }
    if let Some(request) = message.downcast_ref::<SpawnSecondaryWatcherMessage>() {
      if let Some(child) = self.child_slot.lock().as_ref() {
        let watcher_props = Props::from_fn({
          let log = request.log.clone();
          move || SecondaryWatcher::new(log.clone())
        });
        let secondary = ctx
          .spawn_child(&watcher_props)
          .map_err(|error| ActorError::recoverable(format!("spawn secondary failed: {:?}", error)))?;
        secondary.tell(AnyMessage::new(child.clone())).map_err(|_| ActorError::recoverable("link failed"))?;
      }
      return Ok(());
    }
    Ok(())
  }

  fn on_terminated(&mut self, _ctx: &mut ActorContextGeneric<'_, NoStdToolbox>, pid: Pid) -> Result<(), ActorError> {
    self.terminated_log.lock().push(pid);
    self.order_log.lock().push("terminated");
    Ok(())
  }
}

struct SecondaryWatcher {
  log: ArcShared<NoStdMutex<Vec<Pid>>>,
}

impl SecondaryWatcher {
  fn new(log: ArcShared<NoStdMutex<Vec<Pid>>>) -> Self {
    Self { log }
  }
}

impl Actor for SecondaryWatcher {
  fn receive(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, NoStdToolbox>,
    message: AnyMessageView<'_, NoStdToolbox>,
  ) -> Result<(), ActorError> {
    if let Some(child) = message.downcast_ref::<ChildRef>() {
      ctx.watch(child.actor_ref()).map_err(|_| ActorError::recoverable("secondary watch failed"))?;
    }
    Ok(())
  }

  fn on_terminated(&mut self, _ctx: &mut ActorContextGeneric<'_, NoStdToolbox>, pid: Pid) -> Result<(), ActorError> {
    self.log.lock().push(pid);
    Ok(())
  }
}

struct SpawnWatchedGuardian {
  terminated_log: ArcShared<NoStdMutex<Vec<Pid>>>,
}

impl SpawnWatchedGuardian {
  fn new(terminated_log: ArcShared<NoStdMutex<Vec<Pid>>>) -> Self {
    Self { terminated_log }
  }
}

impl Actor for SpawnWatchedGuardian {
  fn receive(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, NoStdToolbox>,
    message: AnyMessageView<'_, NoStdToolbox>,
  ) -> Result<(), ActorError> {
    if message.downcast_ref::<SpawnChild>().is_some() {
      let props = Props::from_fn(|| PassiveChild);
      let child = ctx
        .spawn_child_watched(&props)
        .map_err(|error| ActorError::recoverable(format!("spawn watched failed: {:?}", error)))?;
      child.stop().map_err(|_| ActorError::recoverable("stop failed"))?;
    }
    Ok(())
  }

  fn on_terminated(&mut self, _ctx: &mut ActorContextGeneric<'_, NoStdToolbox>, pid: Pid) -> Result<(), ActorError> {
    self.terminated_log.lock().push(pid);
    Ok(())
  }
}

struct CycleActor {
  log: ArcShared<NoStdMutex<Vec<Pid>>>,
}

impl CycleActor {
  fn new(log: ArcShared<NoStdMutex<Vec<Pid>>>) -> Self {
    Self { log }
  }
}

impl Actor for CycleActor {
  fn receive(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, NoStdToolbox>,
    message: AnyMessageView<'_, NoStdToolbox>,
  ) -> Result<(), ActorError> {
    if let Some(peer) = message.downcast_ref::<ChildRef>() {
      ctx.watch(peer.actor_ref()).map_err(|_| ActorError::recoverable("cycle watch failed"))?;
    } else if message.downcast_ref::<StopChild>().is_some() {
      ctx.stop_self().ok();
    }
    Ok(())
  }

  fn on_terminated(&mut self, _ctx: &mut ActorContextGeneric<'_, NoStdToolbox>, pid: Pid) -> Result<(), ActorError> {
    self.log.lock().push(pid);
    Ok(())
  }
}

struct CycleGuardian {
  log_a: ArcShared<NoStdMutex<Vec<Pid>>>,
  log_b: ArcShared<NoStdMutex<Vec<Pid>>>,
}

impl CycleGuardian {
  fn new(log_a: ArcShared<NoStdMutex<Vec<Pid>>>, log_b: ArcShared<NoStdMutex<Vec<Pid>>>) -> Self {
    Self { log_a, log_b }
  }
}

impl Actor for CycleGuardian {
  fn receive(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, NoStdToolbox>,
    message: AnyMessageView<'_, NoStdToolbox>,
  ) -> Result<(), ActorError> {
    if message.downcast_ref::<StartCycle>().is_some() {
      let actor_a = ctx
        .spawn_child(&Props::from_fn({
          let log_a = self.log_a.clone();
          move || CycleActor::new(log_a.clone())
        }))
        .map_err(|error| ActorError::recoverable(format!("spawn a failed: {:?}", error)))?;
      let actor_b = ctx
        .spawn_child(&Props::from_fn({
          let log_b = self.log_b.clone();
          move || CycleActor::new(log_b.clone())
        }))
        .map_err(|error| ActorError::recoverable(format!("spawn b failed: {:?}", error)))?;
      actor_a.tell(AnyMessage::new(actor_b.clone())).map_err(|_| ActorError::recoverable("link a"))?;
      actor_b.tell(AnyMessage::new(actor_a.clone())).map_err(|_| ActorError::recoverable("link b"))?;
      actor_a.tell(AnyMessage::new(StopChild)).map_err(|_| ActorError::recoverable("stop a"))?;
      actor_b.tell(AnyMessage::new(StopChild)).map_err(|_| ActorError::recoverable("stop b"))?;
    }
    Ok(())
  }
}

fn wait_until(deadline_ms: u64, predicate: &dyn Fn() -> bool) -> bool {
  let deadline = Instant::now() + Duration::from_millis(deadline_ms);
  while Instant::now() < deadline {
    if predicate() {
      return true;
    }
    thread::sleep(Duration::from_millis(5));
  }
  predicate()
}

#[test]
fn death_watch_notifies_parent_on_child_stop() {
  let terminated = ArcShared::new(NoStdMutex::new(Vec::new()));
  let order = ArcShared::new(NoStdMutex::new(Vec::new()));
  let child_slot = ArcShared::new(NoStdMutex::new(None));
  let props = Props::from_fn({
    let terminated = terminated.clone();
    let order = order.clone();
    let child_slot = child_slot.clone();
    move || HarnessWatcher::new(terminated.clone(), order.clone(), child_slot.clone())
  });
  let system = ActorSystem::new(&props).expect("system");

  system.user_guardian_ref().tell(AnyMessage::new(SpawnChild)).expect("spawn child");
  system.user_guardian_ref().tell(AnyMessage::new(StopChild)).expect("stop child");

  let child_pid = child_slot.lock().as_ref().map(|child| child.pid()).unwrap();
  let observed = wait_until(200, &|| terminated.lock().len() == 1);
  let snapshot = terminated.lock().clone();
  assert!(observed, "terminated log={:?}", snapshot);
  assert_eq!(terminated.lock().clone(), vec![child_pid]);
}

#[test]
fn death_watch_unwatch_suppresses_notifications() {
  let terminated = ArcShared::new(NoStdMutex::new(Vec::new()));
  let order = ArcShared::new(NoStdMutex::new(Vec::new()));
  let child_slot = ArcShared::new(NoStdMutex::new(None));
  let props = Props::from_fn({
    let terminated = terminated.clone();
    let order = order.clone();
    let child_slot = child_slot.clone();
    move || HarnessWatcher::new(terminated.clone(), order.clone(), child_slot.clone())
  });
  let system = ActorSystem::new(&props).expect("system");

  system.user_guardian_ref().tell(AnyMessage::new(SpawnChild)).expect("spawn child");
  system.user_guardian_ref().tell(AnyMessage::new(UnwatchChild)).expect("unwatch");
  system.user_guardian_ref().tell(AnyMessage::new(StopChild)).expect("stop child");

  thread::sleep(Duration::from_millis(50));
  assert!(terminated.lock().is_empty());
}

#[test]
fn death_watch_handles_multiple_watchers() {
  let primary_log = ArcShared::new(NoStdMutex::new(Vec::new()));
  let order = ArcShared::new(NoStdMutex::new(Vec::new()));
  let child_slot = ArcShared::new(NoStdMutex::new(None));
  let secondary_log = ArcShared::new(NoStdMutex::new(Vec::new()));
  let props = Props::from_fn({
    let primary_log = primary_log.clone();
    let order = order.clone();
    let child_slot = child_slot.clone();
    move || HarnessWatcher::new(primary_log.clone(), order.clone(), child_slot.clone())
  });
  let system = ActorSystem::new(&props).expect("system");

  system.user_guardian_ref().tell(AnyMessage::new(SpawnChild)).expect("spawn child");
  system
    .user_guardian_ref()
    .tell(AnyMessage::new(SpawnSecondaryWatcherMessage { log: secondary_log.clone() }))
    .expect("spawn secondary");
  system.user_guardian_ref().tell(AnyMessage::new(StopChild)).expect("stop child");

  let pid = child_slot.lock().as_ref().map(|child| child.pid()).unwrap();
  let primary_ready = wait_until(200, &|| primary_log.lock().len() == 1);
  let secondary_ready = wait_until(200, &|| secondary_log.lock().len() == 1);
  assert!(primary_ready && secondary_ready);
  assert_eq!(primary_log.lock().clone(), vec![pid]);
  assert_eq!(secondary_log.lock().clone(), vec![pid]);
}

#[test]
fn watch_after_stop_triggers_immediate_notification() {
  let terminated = ArcShared::new(NoStdMutex::new(Vec::new()));
  let order = ArcShared::new(NoStdMutex::new(Vec::new()));
  let child_slot = ArcShared::new(NoStdMutex::new(None));
  let props = Props::from_fn({
    let terminated = terminated.clone();
    let order = order.clone();
    let child_slot = child_slot.clone();
    move || HarnessWatcher::new(terminated.clone(), order.clone(), child_slot.clone())
  });
  let system = ActorSystem::new(&props).expect("system");

  system.user_guardian_ref().tell(AnyMessage::new(SpawnChild)).expect("spawn child");
  system.user_guardian_ref().tell(AnyMessage::new(StopChild)).expect("stop child");
  let first = wait_until(200, &|| terminated.lock().len() == 1);
  assert!(first);
  terminated.lock().clear();

  system.user_guardian_ref().tell(AnyMessage::new(WatchAfterStop)).expect("rewatch");

  let observed = wait_until(200, &|| terminated.lock().len() == 1);
  assert!(observed);
}

#[test]
fn spawn_child_watched_notifies_on_stop() {
  let terminated = ArcShared::new(NoStdMutex::new(Vec::new()));
  let props = Props::from_fn({
    let terminated = terminated.clone();
    move || SpawnWatchedGuardian::new(terminated.clone())
  });
  let system = ActorSystem::new(&props).expect("system");

  system.user_guardian_ref().tell(AnyMessage::new(SpawnChild)).expect("spawn");
  let observed = wait_until(200, &|| !terminated.lock().is_empty());
  assert!(observed);
}

#[test]
fn terminated_messages_precede_user_queue() {
  let terminated = ArcShared::new(NoStdMutex::new(Vec::new()));
  let order = ArcShared::new(NoStdMutex::new(Vec::new()));
  let child_slot = ArcShared::new(NoStdMutex::new(None));
  let props = Props::from_fn({
    let terminated = terminated.clone();
    let order = order.clone();
    let child_slot = child_slot.clone();
    move || HarnessWatcher::new(terminated.clone(), order.clone(), child_slot.clone())
  });
  let system = ActorSystem::new(&props).expect("system");

  system.user_guardian_ref().tell(AnyMessage::new(SpawnChild)).expect("spawn child");
  system.user_guardian_ref().tell(AnyMessage::new(QueueUserEvent)).expect("queue event");

  let observed = wait_until(200, &|| order.lock().len() >= 2);
  assert!(observed);
  assert_eq!(order.lock().clone(), vec!["terminated", "user"]);
}

#[test]
fn cyclic_watchers_do_not_deadlock() {
  let log_a = ArcShared::new(NoStdMutex::new(Vec::new()));
  let log_b = ArcShared::new(NoStdMutex::new(Vec::new()));
  let props = Props::from_fn({
    let log_a = log_a.clone();
    let log_b = log_b.clone();
    move || CycleGuardian::new(log_a.clone(), log_b.clone())
  });
  let system = ActorSystem::new(&props).expect("system");

  system.user_guardian_ref().tell(AnyMessage::new(StartCycle)).expect("start cycle");

  let observed = wait_until(500, &|| log_b.lock().len() == 1);
  let snapshot_b = log_b.lock().clone();
  assert!(observed, "log_b={:?}", snapshot_b);
}
