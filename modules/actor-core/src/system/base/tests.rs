use alloc::{vec, vec::Vec};

use cellactor_utils_core_rs::sync::{ArcShared, NoStdMutex};

use super::ActorSystem;
use crate::{
  NoStdToolbox,
  actor_prim::Actor,
  dispatcher::{DispatchExecutor, DispatchShared},
  event_stream::{EventStreamEvent, EventStreamSubscriber},
  lifecycle::LifecycleStage,
  messaging::SystemMessage,
  props::{DispatcherConfig, Props},
};

struct TestActor;

impl Actor<NoStdToolbox> for TestActor {
  fn receive(
    &mut self,
    _context: &mut crate::actor_prim::ActorContext<'_, NoStdToolbox>,
    _message: crate::messaging::AnyMessageView<'_, NoStdToolbox>,
  ) -> Result<(), crate::error::ActorError> {
    Ok(())
  }
}

struct SpawnRecorderActor {
  log: ArcShared<NoStdMutex<Vec<&'static str>>>,
}

impl SpawnRecorderActor {
  fn new(log: ArcShared<NoStdMutex<Vec<&'static str>>>) -> Self {
    Self { log }
  }
}

impl Actor<NoStdToolbox> for SpawnRecorderActor {
  fn pre_start(
    &mut self,
    _ctx: &mut crate::actor_prim::ActorContext<'_, NoStdToolbox>,
  ) -> Result<(), crate::error::ActorError> {
    self.log.lock().push("pre_start");
    Ok(())
  }

  fn receive(
    &mut self,
    _context: &mut crate::actor_prim::ActorContext<'_, NoStdToolbox>,
    _message: crate::messaging::AnyMessageView<'_, NoStdToolbox>,
  ) -> Result<(), crate::error::ActorError> {
    self.log.lock().push("receive");
    Ok(())
  }
}

struct FailingStartActor;

impl Actor<NoStdToolbox> for FailingStartActor {
  fn receive(
    &mut self,
    _context: &mut crate::actor_prim::ActorContext<'_, NoStdToolbox>,
    _message: crate::messaging::AnyMessageView<'_, NoStdToolbox>,
  ) -> Result<(), crate::error::ActorError> {
    Ok(())
  }

  fn pre_start(
    &mut self,
    _ctx: &mut crate::actor_prim::ActorContext<'_, NoStdToolbox>,
  ) -> Result<(), crate::error::ActorError> {
    Err(crate::error::ActorError::recoverable("boom"))
  }
}

struct LifecycleEventWatcher {
  stages: ArcShared<NoStdMutex<Vec<LifecycleStage>>>,
}

impl LifecycleEventWatcher {
  fn new(stages: ArcShared<NoStdMutex<Vec<LifecycleStage>>>) -> Self {
    Self { stages }
  }
}

impl EventStreamSubscriber<NoStdToolbox> for LifecycleEventWatcher {
  fn on_event(&self, event: &EventStreamEvent<NoStdToolbox>) {
    if let EventStreamEvent::Lifecycle(lifecycle) = event {
      self.stages.lock().push(lifecycle.stage());
    }
  }
}

struct NoopExecutor;

impl NoopExecutor {
  const fn new() -> Self {
    Self
  }
}

impl DispatchExecutor<NoStdToolbox> for NoopExecutor {
  fn execute(&self, _dispatcher: DispatchShared<NoStdToolbox>) {}
}

#[test]
fn actor_system_new_empty() {
  let system = ActorSystem::new_empty();
  assert!(!system.state().is_terminated());
}

#[test]
fn actor_system_from_state() {
  let state = crate::system::system_state::SystemState::new();
  let system = ActorSystem::from_state(ArcShared::new(state));
  assert!(!system.state().is_terminated());
}

#[test]
fn actor_system_clone() {
  let system1 = ActorSystem::new_empty();
  let system2 = system1.clone();
  assert!(!system1.state().is_terminated());
  assert!(!system2.state().is_terminated());
}

#[test]
fn actor_system_allocate_pid() {
  let system = ActorSystem::new_empty();
  let pid1 = system.allocate_pid();
  let pid2 = system.allocate_pid();
  assert_ne!(pid1.value(), pid2.value());
}

#[test]
fn actor_system_state() {
  let system = ActorSystem::new_empty();
  let state = system.state();
  assert!(!state.is_terminated());
}

#[test]
fn actor_system_event_stream() {
  let system = ActorSystem::new_empty();
  let stream = system.event_stream();
  let _ = stream;
}

#[test]
fn actor_system_deadletters() {
  let system = ActorSystem::new_empty();
  let deadletters = system.dead_letters();
  assert_eq!(deadletters.len(), 0);
}

#[test]
fn actor_system_emit_log() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  system.emit_log(crate::logging::LogLevel::Info, "test message", Some(pid));
}

#[test]
fn actor_system_when_terminated() {
  let system = ActorSystem::new_empty();
  let future = system.when_terminated();
  assert!(!future.is_ready());
}

#[test]
fn actor_system_actor_ref_for_nonexistent_pid() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  assert!(system.actor_ref(pid).is_none());
}

#[test]
fn actor_system_children_for_nonexistent_parent() {
  let system = ActorSystem::new_empty();
  let parent_pid = system.allocate_pid();
  let children = system.children(parent_pid);
  assert_eq!(children.len(), 0);
}

#[test]
fn actor_system_spawn_child_with_invalid_parent() {
  let system = ActorSystem::new_empty();
  let props = Props::from_fn(|| TestActor);
  let invalid_parent = system.allocate_pid();

  let result = system.spawn_child(invalid_parent, &props);
  assert!(result.is_err());
}

#[test]
fn actor_system_spawn_without_guardian() {
  let system = ActorSystem::new_empty();
  let props = Props::from_fn(|| TestActor);

  let result = system.spawn(&props);
  assert!(result.is_err());
}

#[test]
fn actor_system_drain_ready_ask_futures() {
  let system = ActorSystem::new_empty();
  let futures = system.drain_ready_ask_futures();
  assert_eq!(futures.len(), 0);
}

#[test]
fn actor_system_terminate_without_guardian() {
  let system = ActorSystem::new_empty();
  let result = system.terminate();
  assert!(result.is_ok());
  assert!(system.state().is_terminated());
}

#[test]
fn actor_system_terminate_when_already_terminated() {
  let system = ActorSystem::new_empty();
  system.state().mark_terminated();
  let result = system.terminate();
  assert!(result.is_ok());
}

#[test]
fn spawn_does_not_block_when_dispatcher_never_runs() {
  let system = ActorSystem::new_empty();
  let log: ArcShared<NoStdMutex<Vec<&'static str>>> = ArcShared::new(NoStdMutex::new(Vec::new()));
  let props = Props::from_fn({
    let log = log.clone();
    move || SpawnRecorderActor::new(log.clone())
  })
  .with_dispatcher(DispatcherConfig::from_executor(ArcShared::new(NoopExecutor::new())));

  let child = system.spawn_with_parent(None, &props).expect("spawn succeeds");
  assert!(log.lock().is_empty());
  assert!(system.state().cell(&child.pid()).is_some());
}

#[test]
fn spawn_succeeds_even_if_pre_start_fails() {
  let system = ActorSystem::new_empty();
  let props = Props::from_fn(|| FailingStartActor);
  let child = system.spawn_with_parent(None, &props).expect("spawn succeeds despite failure");

  assert!(system.state().cell(&child.pid()).is_none());
}

#[test]
fn create_send_failure_triggers_rollback() {
  let system = ActorSystem::new_empty();
  let props = Props::from_fn(|| TestActor);
  let pid = system.allocate_pid();
  let name = system.state().assign_name(None, props.name(), pid).expect("name assigned");
  let cell = system.build_cell_for_spawn(pid, None, name, &props);
  system.state().register_cell(cell.clone());

  system.state().remove_cell(&pid);
  let result = system.perform_create_handshake(None, pid, &cell);

  match result {
    | Err(crate::spawn::SpawnError::InvalidProps(reason)) => {
      assert_eq!(reason, super::CREATE_SEND_FAILED);
    },
    | other => panic!("unexpected handshake result: {:?}", other),
  }

  assert!(system.state().cell(&pid).is_none());
  let retry = system.state().assign_name(None, Some(cell.name()), pid);
  assert!(retry.is_ok());
}

#[test]
fn spawn_returns_child_ref_even_if_dispatcher_is_idle() {
  let system = ActorSystem::new_empty();
  let props =
    Props::from_fn(|| TestActor).with_dispatcher(DispatcherConfig::from_executor(ArcShared::new(NoopExecutor::new())));
  let result = system.spawn_with_parent(None, &props);

  assert!(result.is_ok());
}

#[test]
fn lifecycle_events_cover_restart_transitions() {
  let system = ActorSystem::new_empty();
  let stages: ArcShared<NoStdMutex<Vec<LifecycleStage>>> = ArcShared::new(NoStdMutex::new(Vec::new()));
  let subscriber_impl = ArcShared::new(LifecycleEventWatcher::new(stages.clone()));
  let subscriber: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = subscriber_impl;
  let _subscription = system.subscribe_event_stream(&subscriber);

  let props = Props::from_fn(|| TestActor);
  let child = system.spawn_with_parent(None, &props).expect("spawn succeeds");
  let pid = child.pid();

  system.state().send_system_message(pid, SystemMessage::Recreate).expect("recreate enqueued");

  let snapshot = stages.lock().clone();
  assert_eq!(snapshot, vec![LifecycleStage::Started, LifecycleStage::Stopped, LifecycleStage::Restarted]);
}
