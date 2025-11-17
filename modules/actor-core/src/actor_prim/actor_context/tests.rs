use alloc::{string::String, vec, vec::Vec};
use core::hint::spin_loop;

use fraktor_utils_core_rs::core::sync::{ArcShared, NoStdMutex};

use super::{ActorContext, ActorContextGeneric};
use crate::{
  NoStdToolbox,
  actor_prim::{Actor, ActorCell, Pid},
  messaging::{AnyMessage, AnyMessageViewGeneric},
  props::Props,
  system::ActorSystem,
};

struct TestActor;

impl Actor for TestActor {
  fn receive(
    &mut self,
    _context: &mut ActorContextGeneric<'_, NoStdToolbox>,
    _message: AnyMessageViewGeneric<'_, NoStdToolbox>,
  ) -> Result<(), crate::error::ActorError> {
    Ok(())
  }
}

struct RecordingActor {
  log: ArcShared<NoStdMutex<Vec<Pid>>>,
}

impl RecordingActor {
  fn new(log: ArcShared<NoStdMutex<Vec<Pid>>>) -> Self {
    Self { log }
  }
}

impl Actor for RecordingActor {
  fn receive(
    &mut self,
    _context: &mut ActorContextGeneric<'_, NoStdToolbox>,
    _message: AnyMessageViewGeneric<'_, NoStdToolbox>,
  ) -> Result<(), crate::error::ActorError> {
    Ok(())
  }

  fn on_terminated(
    &mut self,
    _ctx: &mut ActorContextGeneric<'_, NoStdToolbox>,
    pid: Pid,
  ) -> Result<(), crate::error::ActorError> {
    self.log.lock().push(pid);
    Ok(())
  }
}

struct ProbeActor {
  received: ArcShared<NoStdMutex<Vec<i32>>>,
}

impl ProbeActor {
  fn new(received: ArcShared<NoStdMutex<Vec<i32>>>) -> Self {
    Self { received }
  }
}

impl Actor for ProbeActor {
  fn receive(
    &mut self,
    _context: &mut ActorContextGeneric<'_, NoStdToolbox>,
    message: AnyMessageViewGeneric<'_, NoStdToolbox>,
  ) -> Result<(), crate::error::ActorError> {
    if let Some(value) = message.downcast_ref::<i32>() {
      self.received.lock().push(*value);
    }
    Ok(())
  }
}

#[test]
fn actor_context_new() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let context = ActorContext::new(&system, pid);
  assert_eq!(context.pid(), pid);
}

#[test]
fn actor_context_system() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let context = ActorContext::new(&system, pid);
  let retrieved_system = context.system();
  let _ = retrieved_system;
}

#[test]
fn actor_context_pid() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let context = ActorContext::new(&system, pid);
  assert_eq!(context.pid(), pid);
}

#[test]
fn actor_context_reply_to_initially_none() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let context = ActorContext::new(&system, pid);
  assert!(context.reply_to().is_none());
}

#[test]
fn actor_context_set_and_clear_reply_to() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);

  assert!(context.reply_to().is_none());

  context.clear_reply_to();
  assert!(context.reply_to().is_none());
}

#[test]
fn actor_context_reply_without_reply_to() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let context = ActorContext::new(&system, pid);

  let result = context.reply(AnyMessage::new(42_u32));
  assert!(result.is_err());
}

#[test]
fn actor_context_children() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let context = ActorContext::new(&system, pid);

  let children = context.children();
  assert_eq!(children.len(), 0);
}

#[test]
fn actor_context_spawn_child_with_invalid_parent() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let context = ActorContext::new(&system, pid);
  let props = Props::from_fn(|| TestActor);

  let result = context.spawn_child(&props);
  assert!(result.is_err());
}

#[test]
fn actor_context_log() {
  use alloc::string::String;

  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let context = ActorContext::new(&system, pid);

  context.log(crate::logging::LogLevel::Info, String::from("test message"));
  context.log(crate::logging::LogLevel::Error, String::from("error message"));
}

fn wait_until(mut condition: impl FnMut() -> bool) {
  for _ in 0..10_000 {
    if condition() {
      return;
    }
    spin_loop();
  }
  assert!(condition());
}

#[test]
fn actor_context_pipe_to_self_enqueues_message() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let received = ArcShared::new(NoStdMutex::new(Vec::new()));
  let props = Props::from_fn({
    let log = received.clone();
    move || ProbeActor::new(log.clone())
  });
  register_cell(&system, pid, "self", &props);
  let context = ActorContext::new(&system, pid);

  context.pipe_to_self(async { 41_i32 }, AnyMessage::new).expect("pipe to self");

  wait_until(|| !received.lock().is_empty());
  assert_eq!(received.lock()[0], 41);
}

#[test]
fn actor_context_pipe_to_self_handles_async_future() {
  use crate::futures::ActorFuture;

  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let received = ArcShared::new(NoStdMutex::new(Vec::new()));
  let props = Props::from_fn({
    let log = received.clone();
    move || ProbeActor::new(log.clone())
  });
  register_cell(&system, pid, "self", &props);
  let context = ActorContext::new(&system, pid);

  let signal = ArcShared::new(ActorFuture::<i32>::new());
  let future = {
    let handle = signal.clone();
    async move { handle.listener().await }
  };

  context.pipe_to_self(future, AnyMessage::new).expect("pipe to self");
  assert!(received.lock().is_empty());

  signal.complete(7);
  wait_until(|| !received.lock().is_empty());
  assert_eq!(received.lock()[0], 7);
}

fn register_cell(system: &ActorSystem, pid: Pid, name: &str, props: &Props) -> ArcShared<ActorCell> {
  let cell = ActorCell::create(system.state(), pid, None, String::from(name), props).expect("create actor cell");
  system.state().register_cell(cell.clone());
  cell
}

#[test]
fn actor_context_watch_enqueues_system_message() {
  let system = ActorSystem::new_empty();
  let watcher_pid = system.allocate_pid();
  let target_pid = system.allocate_pid();
  let props = Props::from_fn(|| TestActor);
  let _watcher = register_cell(&system, watcher_pid, "watcher", &props);
  let target = register_cell(&system, target_pid, "target", &props);

  let context = ActorContext::new(&system, watcher_pid);
  let target_ref = target.actor_ref();
  assert!(context.watch(&target_ref).is_ok());
  assert!(target.watchers_snapshot().contains(&watcher_pid));
}

#[test]
fn actor_context_watch_missing_actor_notifies_self() {
  let system = ActorSystem::new_empty();
  let watcher_pid = system.allocate_pid();
  let target_pid = system.allocate_pid();
  let watcher_log = ArcShared::new(NoStdMutex::new(Vec::new()));
  let watcher_props = Props::from_fn({
    let log = watcher_log.clone();
    move || RecordingActor::new(log.clone())
  });
  let target_props = Props::from_fn(|| TestActor);
  let _watcher = register_cell(&system, watcher_pid, "watcher", &watcher_props);
  let target = register_cell(&system, target_pid, "target", &target_props);
  let target_ref = target.actor_ref();
  system.state().remove_cell(&target_pid);

  let context = ActorContext::new(&system, watcher_pid);
  assert!(context.watch(&target_ref).is_ok());
  assert_eq!(watcher_log.lock().clone(), vec![target_pid]);
}

#[test]
fn actor_context_unwatch_enqueues_message() {
  let system = ActorSystem::new_empty();
  let watcher_pid = system.allocate_pid();
  let target_pid = system.allocate_pid();
  let props = Props::from_fn(|| TestActor);
  let _watcher = register_cell(&system, watcher_pid, "watcher", &props);
  let target = register_cell(&system, target_pid, "target", &props);
  let context = ActorContext::new(&system, watcher_pid);
  let target_ref = target.actor_ref();

  assert!(context.watch(&target_ref).is_ok());
  assert!(context.unwatch(&target_ref).is_ok());
  assert!(!target.watchers_snapshot().contains(&watcher_pid));
}

#[test]
fn spawn_child_watched_installs_watch() {
  let system = ActorSystem::new_empty();
  let parent_pid = system.allocate_pid();
  let props = Props::from_fn(|| TestActor);
  let _parent = register_cell(&system, parent_pid, "parent", &props);
  let context = ActorContext::new(&system, parent_pid);
  let child_props = Props::from_fn(|| TestActor);

  let child = context.spawn_child_watched(&child_props).expect("child spawn succeeds");
  let child_cell = system.state().cell(&child.pid()).expect("child registered");

  assert!(child_cell.watchers_snapshot().contains(&parent_pid));
}
