#![cfg(not(target_os = "none"))]

use std::{
  thread,
  time::{Duration, Instant},
};

use fraktor_actor_core_rs::core::kernel::{
  actor::{
    Actor, ActorContext, ChildRef, Pid,
    error::ActorError,
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
    scheduler::tick_driver::TestTickDriver,
    setup::ActorSystemConfig,
  },
  system::ActorSystem,
};
use fraktor_utils_core_rs::core::sync::{ArcShared, SpinSyncMutex};

struct SpawnChild;
struct StopChild;
struct UnwatchChild;
struct WatchAfterStop;
struct QueueUserEvent;
struct SpawnSecondaryWatcherMessage {
  log: ArcShared<SpinSyncMutex<Vec<Pid>>>,
}
struct StartCycle;
struct UserProbe;

struct PassiveChild;

impl Actor for PassiveChild {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<StopChild>().is_some() {
      ctx.stop_self().ok();
    }
    Ok(())
  }
}

struct HarnessWatcher {
  terminated_log: ArcShared<SpinSyncMutex<Vec<Pid>>>,
  order_log:      ArcShared<SpinSyncMutex<Vec<&'static str>>>,
  child_slot:     ArcShared<SpinSyncMutex<Option<ChildRef>>>,
}

impl HarnessWatcher {
  fn new(
    terminated_log: ArcShared<SpinSyncMutex<Vec<Pid>>>,
    order_log: ArcShared<SpinSyncMutex<Vec<&'static str>>>,
    child_slot: ArcShared<SpinSyncMutex<Option<ChildRef>>>,
  ) -> Self {
    Self { terminated_log, order_log, child_slot }
  }

  fn spawn_child(&self, ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
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
      child.stop().expect("stop child should succeed");
    }
  }
}

impl Actor for HarnessWatcher {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
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
      ctx.self_ref().tell(AnyMessage::new(UserProbe));
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
        let mut secondary = ctx
          .spawn_child(&watcher_props)
          .map_err(|error| ActorError::recoverable(format!("spawn secondary failed: {:?}", error)))?;
        secondary.tell(AnyMessage::new(child.clone()));
      }
      return Ok(());
    }
    Ok(())
  }

  fn on_terminated(&mut self, _ctx: &mut ActorContext<'_>, pid: Pid) -> Result<(), ActorError> {
    self.terminated_log.lock().push(pid);
    self.order_log.lock().push("terminated");
    Ok(())
  }
}

struct SecondaryWatcher {
  log: ArcShared<SpinSyncMutex<Vec<Pid>>>,
}

impl SecondaryWatcher {
  fn new(log: ArcShared<SpinSyncMutex<Vec<Pid>>>) -> Self {
    Self { log }
  }
}

impl Actor for SecondaryWatcher {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(child) = message.downcast_ref::<ChildRef>() {
      ctx.watch(child.actor_ref()).map_err(|_| ActorError::recoverable("secondary watch failed"))?;
    }
    Ok(())
  }

  fn on_terminated(&mut self, _ctx: &mut ActorContext<'_>, pid: Pid) -> Result<(), ActorError> {
    self.log.lock().push(pid);
    Ok(())
  }
}

struct SpawnWatchedGuardian {
  terminated_log: ArcShared<SpinSyncMutex<Vec<Pid>>>,
}

impl SpawnWatchedGuardian {
  fn new(terminated_log: ArcShared<SpinSyncMutex<Vec<Pid>>>) -> Self {
    Self { terminated_log }
  }
}

impl Actor for SpawnWatchedGuardian {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<SpawnChild>().is_some() {
      let props = Props::from_fn(|| PassiveChild);
      let child = ctx
        .spawn_child_watched(&props)
        .map_err(|error| ActorError::recoverable(format!("spawn watched failed: {:?}", error)))?;
      child.stop().map_err(|_| ActorError::recoverable("stop failed"))?;
    }
    Ok(())
  }

  fn on_terminated(&mut self, _ctx: &mut ActorContext<'_>, pid: Pid) -> Result<(), ActorError> {
    self.terminated_log.lock().push(pid);
    Ok(())
  }
}

struct CycleActor {
  log: ArcShared<SpinSyncMutex<Vec<Pid>>>,
}

impl CycleActor {
  fn new(log: ArcShared<SpinSyncMutex<Vec<Pid>>>) -> Self {
    Self { log }
  }
}

impl Actor for CycleActor {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(peer) = message.downcast_ref::<ChildRef>() {
      ctx.watch(peer.actor_ref()).map_err(|_| ActorError::recoverable("cycle watch failed"))?;
    } else if message.downcast_ref::<StopChild>().is_some() {
      ctx.stop_self().ok();
    }
    Ok(())
  }

  fn on_terminated(&mut self, _ctx: &mut ActorContext<'_>, pid: Pid) -> Result<(), ActorError> {
    self.log.lock().push(pid);
    Ok(())
  }
}

struct CycleGuardian {
  log_a: ArcShared<SpinSyncMutex<Vec<Pid>>>,
  log_b: ArcShared<SpinSyncMutex<Vec<Pid>>>,
}

impl CycleGuardian {
  fn new(log_a: ArcShared<SpinSyncMutex<Vec<Pid>>>, log_b: ArcShared<SpinSyncMutex<Vec<Pid>>>) -> Self {
    Self { log_a, log_b }
  }
}

impl Actor for CycleGuardian {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
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
      let actor_a_child = actor_a.clone();
      let actor_b_child = actor_b.clone();
      let mut actor_a_ref = actor_a.into_actor_ref();
      actor_a_ref.tell(AnyMessage::new(actor_b_child));
      let mut actor_b_ref = actor_b.into_actor_ref();
      actor_b_ref.tell(AnyMessage::new(actor_a_child));
      actor_a_ref.tell(AnyMessage::new(StopChild));
      actor_b_ref.tell(AnyMessage::new(StopChild));
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
  let terminated = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let order = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let child_slot = ArcShared::new(SpinSyncMutex::new(None));
  let props = Props::from_fn({
    let terminated = terminated.clone();
    let order = order.clone();
    let child_slot = child_slot.clone();
    move || HarnessWatcher::new(terminated.clone(), order.clone(), child_slot.clone())
  });
  let system =
    ActorSystem::create_with_config(&props, ActorSystemConfig::new(TestTickDriver::default())).expect("system");

  system.user_guardian_ref().tell(AnyMessage::new(SpawnChild));
  system.user_guardian_ref().tell(AnyMessage::new(StopChild));

  let child_pid = child_slot.lock().as_ref().map(|child| child.pid()).unwrap();
  let observed = wait_until(200, &|| terminated.lock().len() == 1);
  let snapshot = terminated.lock().clone();
  assert!(observed, "terminated log={:?}", snapshot);
  assert_eq!(terminated.lock().clone(), vec![child_pid]);
}

#[test]
fn death_watch_unwatch_suppresses_notifications() {
  let terminated = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let order = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let child_slot = ArcShared::new(SpinSyncMutex::new(None));
  let props = Props::from_fn({
    let terminated = terminated.clone();
    let order = order.clone();
    let child_slot = child_slot.clone();
    move || HarnessWatcher::new(terminated.clone(), order.clone(), child_slot.clone())
  });
  let system =
    ActorSystem::create_with_config(&props, ActorSystemConfig::new(TestTickDriver::default())).expect("system");

  system.user_guardian_ref().tell(AnyMessage::new(SpawnChild));
  system.user_guardian_ref().tell(AnyMessage::new(UnwatchChild));
  system.user_guardian_ref().tell(AnyMessage::new(StopChild));

  thread::sleep(Duration::from_millis(50));
  assert!(terminated.lock().is_empty());
}

#[test]
fn death_watch_handles_multiple_watchers() {
  let primary_log = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let order = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let child_slot = ArcShared::new(SpinSyncMutex::new(None));
  let secondary_log = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let props = Props::from_fn({
    let primary_log = primary_log.clone();
    let order = order.clone();
    let child_slot = child_slot.clone();
    move || HarnessWatcher::new(primary_log.clone(), order.clone(), child_slot.clone())
  });
  let system =
    ActorSystem::create_with_config(&props, ActorSystemConfig::new(TestTickDriver::default())).expect("system");

  system.user_guardian_ref().tell(AnyMessage::new(SpawnChild));
  system.user_guardian_ref().tell(AnyMessage::new(SpawnSecondaryWatcherMessage { log: secondary_log.clone() }));
  system.user_guardian_ref().tell(AnyMessage::new(StopChild));

  let pid = child_slot.lock().as_ref().map(|child| child.pid()).unwrap();
  let primary_ready = wait_until(200, &|| primary_log.lock().len() == 1);
  let secondary_ready = wait_until(200, &|| secondary_log.lock().len() == 1);
  assert!(primary_ready && secondary_ready);
  assert_eq!(primary_log.lock().clone(), vec![pid]);
  assert_eq!(secondary_log.lock().clone(), vec![pid]);
}

#[test]
fn watch_after_stop_triggers_immediate_notification() {
  let terminated = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let order = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let child_slot = ArcShared::new(SpinSyncMutex::new(None));
  let props = Props::from_fn({
    let terminated = terminated.clone();
    let order = order.clone();
    let child_slot = child_slot.clone();
    move || HarnessWatcher::new(terminated.clone(), order.clone(), child_slot.clone())
  });
  let system =
    ActorSystem::create_with_config(&props, ActorSystemConfig::new(TestTickDriver::default())).expect("system");

  system.user_guardian_ref().tell(AnyMessage::new(SpawnChild));
  system.user_guardian_ref().tell(AnyMessage::new(StopChild));
  let first = wait_until(200, &|| terminated.lock().len() == 1);
  assert!(first);
  terminated.lock().clear();

  system.user_guardian_ref().tell(AnyMessage::new(WatchAfterStop));

  let observed = wait_until(200, &|| terminated.lock().len() == 1);
  assert!(observed);
}

#[test]
fn spawn_child_watched_notifies_on_stop() {
  let terminated = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let props = Props::from_fn({
    let terminated = terminated.clone();
    move || SpawnWatchedGuardian::new(terminated.clone())
  });
  let system =
    ActorSystem::create_with_config(&props, ActorSystemConfig::new(TestTickDriver::default())).expect("system");

  system.user_guardian_ref().tell(AnyMessage::new(SpawnChild));
  let observed = wait_until(200, &|| !terminated.lock().is_empty());
  assert!(observed);
}

/// Verifies that both user messages and termination notifications are processed.
///
/// Note: The relative ordering between `Terminated` system messages and user messages
/// is only guaranteed when both are present in the mailbox at dequeue time.
/// With async executor (DispatchExecutorRunner), child stopping is queued and may
/// execute after the parent processes pending user messages. This is expected
/// behavior in an async actor system where cross-actor timing is non-deterministic.
#[test]
fn terminated_and_user_messages_are_both_processed() {
  let terminated = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let order = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let child_slot = ArcShared::new(SpinSyncMutex::new(None));
  let props = Props::from_fn({
    let terminated = terminated.clone();
    let order = order.clone();
    let child_slot = child_slot.clone();
    move || HarnessWatcher::new(terminated.clone(), order.clone(), child_slot.clone())
  });
  let system =
    ActorSystem::create_with_config(&props, ActorSystemConfig::new(TestTickDriver::default())).expect("system");

  system.user_guardian_ref().tell(AnyMessage::new(SpawnChild));
  system.user_guardian_ref().tell(AnyMessage::new(QueueUserEvent));

  let observed = wait_until(200, &|| order.lock().len() >= 2);
  assert!(observed);
  // Both "terminated" and "user" should be present, order is timing-dependent
  let order_snapshot = order.lock().clone();
  assert!(order_snapshot.contains(&"terminated"), "terminated event missing");
  assert!(order_snapshot.contains(&"user"), "user event missing");
}

#[test]
fn cyclic_watchers_do_not_deadlock() {
  let log_a = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let log_b = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let props = Props::from_fn({
    let log_a = log_a.clone();
    let log_b = log_b.clone();
    move || CycleGuardian::new(log_a.clone(), log_b.clone())
  });
  let system =
    ActorSystem::create_with_config(&props, ActorSystemConfig::new(TestTickDriver::default())).expect("system");

  system.user_guardian_ref().tell(AnyMessage::new(StartCycle));

  let observed = wait_until(500, &|| log_b.lock().len() == 1);
  let snapshot_b = log_b.lock().clone();
  assert!(observed, "log_b={:?}", snapshot_b);
}

// --- watch_with tests ---

struct WatchWithNotification {
  terminated_pid: Pid,
}

struct WatchWithHarness {
  custom_log:     ArcShared<SpinSyncMutex<Vec<Pid>>>,
  terminated_log: ArcShared<SpinSyncMutex<Vec<Pid>>>,
  child_slot:     ArcShared<SpinSyncMutex<Option<ChildRef>>>,
}

impl WatchWithHarness {
  fn new(
    custom_log: ArcShared<SpinSyncMutex<Vec<Pid>>>,
    terminated_log: ArcShared<SpinSyncMutex<Vec<Pid>>>,
    child_slot: ArcShared<SpinSyncMutex<Option<ChildRef>>>,
  ) -> Self {
    Self { custom_log, terminated_log, child_slot }
  }
}

impl Actor for WatchWithHarness {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<SpawnChild>().is_some() {
      if self.child_slot.lock().is_some() {
        return Ok(());
      }
      let props = Props::from_fn(|| PassiveChild);
      let child =
        ctx.spawn_child(&props).map_err(|error| ActorError::recoverable(format!("spawn failed: {:?}", error)))?;
      let custom_msg = AnyMessage::new(WatchWithNotification { terminated_pid: child.pid() });
      ctx.watch_with(child.actor_ref(), custom_msg).map_err(|_| ActorError::recoverable("watch_with failed"))?;
      self.child_slot.lock().replace(child);
      return Ok(());
    }
    if message.downcast_ref::<StopChild>().is_some() {
      if let Some(child) = self.child_slot.lock().as_ref() {
        child.stop().expect("stop child should succeed");
      }
      return Ok(());
    }
    if message.downcast_ref::<UnwatchChild>().is_some() {
      if let Some(child) = self.child_slot.lock().as_ref() {
        ctx.unwatch(child.actor_ref()).map_err(|_| ActorError::recoverable("unwatch failed"))?;
      }
      return Ok(());
    }
    if let Some(notification) = message.downcast_ref::<WatchWithNotification>() {
      self.custom_log.lock().push(notification.terminated_pid);
      return Ok(());
    }
    Ok(())
  }

  fn on_terminated(&mut self, _ctx: &mut ActorContext<'_>, pid: Pid) -> Result<(), ActorError> {
    self.terminated_log.lock().push(pid);
    Ok(())
  }
}

#[test]
fn watch_with_delivers_custom_message_instead_of_on_terminated() {
  let custom_log = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let terminated_log = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let child_slot = ArcShared::new(SpinSyncMutex::new(None));
  let props = Props::from_fn({
    let custom_log = custom_log.clone();
    let terminated_log = terminated_log.clone();
    let child_slot = child_slot.clone();
    move || WatchWithHarness::new(custom_log.clone(), terminated_log.clone(), child_slot.clone())
  });
  let system =
    ActorSystem::create_with_config(&props, ActorSystemConfig::new(TestTickDriver::default())).expect("system");

  system.user_guardian_ref().tell(AnyMessage::new(SpawnChild));
  system.user_guardian_ref().tell(AnyMessage::new(StopChild));

  let child_pid = child_slot.lock().as_ref().map(|c| c.pid()).unwrap();
  let observed = wait_until(200, &|| custom_log.lock().len() == 1);
  assert!(observed, "custom message should be delivered via watch_with");
  assert_eq!(custom_log.lock().clone(), vec![child_pid]);
  assert!(terminated_log.lock().is_empty(), "on_terminated should not be called when watch_with is active");
}

#[test]
fn watch_with_unwatch_clears_custom_message_registration() {
  let custom_log = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let terminated_log = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let child_slot = ArcShared::new(SpinSyncMutex::new(None));
  let props = Props::from_fn({
    let custom_log = custom_log.clone();
    let terminated_log = terminated_log.clone();
    let child_slot = child_slot.clone();
    move || WatchWithHarness::new(custom_log.clone(), terminated_log.clone(), child_slot.clone())
  });
  let system =
    ActorSystem::create_with_config(&props, ActorSystemConfig::new(TestTickDriver::default())).expect("system");

  system.user_guardian_ref().tell(AnyMessage::new(SpawnChild));
  system.user_guardian_ref().tell(AnyMessage::new(UnwatchChild));
  system.user_guardian_ref().tell(AnyMessage::new(StopChild));

  thread::sleep(Duration::from_millis(50));
  assert!(custom_log.lock().is_empty(), "no custom message after unwatch");
  assert!(terminated_log.lock().is_empty(), "no on_terminated after unwatch");
}
