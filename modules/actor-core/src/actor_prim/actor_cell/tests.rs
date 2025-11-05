use alloc::{string::ToString, vec, vec::Vec};

use cellactor_utils_core_rs::sync::{ArcShared, NoStdMutex};

use super::ActorCell;
use crate::{
  actor_prim::{Actor, ActorContext, Pid},
  error::ActorError,
  messaging::AnyMessageView,
  props::Props,
  system::SystemState,
};

struct ProbeActor;

impl Actor for ProbeActor {
  fn receive(
    &mut self,
    _ctx: &mut ActorContext<'_, crate::NoStdToolbox>,
    _message: AnyMessageView<'_, crate::NoStdToolbox>,
  ) -> Result<(), ActorError> {
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
    _ctx: &mut ActorContext<'_, crate::NoStdToolbox>,
    _message: AnyMessageView<'_, crate::NoStdToolbox>,
  ) -> Result<(), ActorError> {
    Ok(())
  }

  fn on_terminated(&mut self, _ctx: &mut ActorContext<'_, crate::NoStdToolbox>, pid: Pid) -> Result<(), ActorError> {
    self.log.lock().push(pid);
    Ok(())
  }
}

#[test]
fn actor_cell_holds_components() {
  let system = ArcShared::new(SystemState::new());
  let props = Props::from_fn(|| ProbeActor);
  let cell = ActorCell::create(system, Pid::new(1, 0), None, "worker".to_string(), &props);

  assert_eq!(cell.pid(), Pid::new(1, 0));
  assert_eq!(cell.name(), "worker");
  assert!(cell.parent().is_none());
  assert_eq!(cell.mailbox().system_len(), 0);
  assert_eq!(cell.dispatcher().mailbox().system_len(), 0);
}

#[test]
fn handle_watch_is_idempotent() {
  let system = ArcShared::new(SystemState::new());
  let props = Props::from_fn(|| ProbeActor);
  let target = ActorCell::create(system.clone(), Pid::new(10, 0), None, "target".to_string(), &props);
  system.register_cell(target.clone());

  target.handle_watch(Pid::new(20, 0));
  target.handle_watch(Pid::new(20, 0));

  assert_eq!(target.watchers.lock().len(), 1);
}

#[test]
fn handle_unwatch_removes_pid() {
  let system = ArcShared::new(SystemState::new());
  let props = Props::from_fn(|| ProbeActor);
  let target = ActorCell::create(system.clone(), Pid::new(11, 0), None, "target".to_string(), &props);
  system.register_cell(target.clone());

  target.handle_watch(Pid::new(21, 0));
  target.handle_unwatch(Pid::new(21, 0));

  assert_eq!(target.watchers.lock().len(), 0);
}

#[test]
fn notify_watchers_sends_terminated() {
  let state = ArcShared::new(SystemState::new());
  let props = Props::from_fn(|| ProbeActor);
  let target = ActorCell::create(state.clone(), Pid::new(30, 0), None, "target".to_string(), &props);
  let log = ArcShared::new(NoStdMutex::new(Vec::new()));
  let watcher_props = Props::from_fn({
    let log = log.clone();
    move || RecordingActor::new(log.clone())
  });
  let watcher = ActorCell::create(state.clone(), Pid::new(31, 0), None, "watcher".to_string(), &watcher_props);
  state.register_cell(target.clone());
  state.register_cell(watcher.clone());

  target.handle_watch(watcher.pid());
  target.notify_watchers_on_stop();
  assert_eq!(log.lock().clone(), vec![target.pid()]);
  assert_eq!(target.watchers.lock().len(), 0);
}
