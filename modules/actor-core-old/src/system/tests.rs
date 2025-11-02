use alloc::{vec, vec::Vec};

use cellactor_utils_core_rs::sync::ArcShared;

use super::ActorSystem;
use crate::{
  ActorRuntimeMutex,
  actor::Actor,
  actor_context::ActorContext,
  actor_error::{ActorError, ActorErrorReason},
  any_message::{AnyMessage, AnyMessageView},
  child_ref::ChildRef,
  pid::Pid,
  props::Props,
};

struct Start;

struct GuardianLogger {
  log: ArcShared<ActorRuntimeMutex<Vec<&'static str>>>,
}

impl GuardianLogger {
  fn new(log: ArcShared<ActorRuntimeMutex<Vec<&'static str>>>) -> Self {
    Self { log }
  }
}

impl Actor for GuardianLogger {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_some() {
      self.log.lock().push("start");
    }
    Ok(())
  }
}

#[test]
fn guardian_processes_message() {
  let log = ArcShared::new(ActorRuntimeMutex::new(Vec::new()));
  let props = Props::from_fn({
    let log = log.clone();
    move || GuardianLogger::new(log.clone())
  });
  let system = ActorSystem::new(&props).expect("create system");

  system.user_guardian_ref().tell(AnyMessage::new(Start)).expect("send");

  assert_eq!(log.lock().clone(), vec!["start"]);
}

struct ChildRecorder {
  log: ArcShared<ActorRuntimeMutex<Vec<u32>>>,
}

impl ChildRecorder {
  fn new(log: ArcShared<ActorRuntimeMutex<Vec<u32>>>) -> Self {
    Self { log }
  }
}

impl Actor for ChildRecorder {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(value) = message.downcast_ref::<u32>() {
      self.log.lock().push(*value);
    }
    Ok(())
  }
}

struct ChildSpawner {
  child_slot: ArcShared<ActorRuntimeMutex<Option<ChildRef>>>,
  log:        ArcShared<ActorRuntimeMutex<Vec<u32>>>,
}

impl ChildSpawner {
  fn new(
    child_slot: ArcShared<ActorRuntimeMutex<Option<ChildRef>>>,
    log: ArcShared<ActorRuntimeMutex<Vec<u32>>>,
  ) -> Self {
    Self { child_slot, log }
  }
}

impl Actor for ChildSpawner {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_some() && self.child_slot.lock().is_none() {
      let props = Props::from_fn({
        let log = self.log.clone();
        move || ChildRecorder::new(log.clone())
      });
      let child =
        ctx.spawn_child(&props).map_err(|_| ActorError::recoverable(ActorErrorReason::new("spawn failed")))?;
      self.child_slot.lock().replace(child);
    }
    Ok(())
  }
}

#[test]
fn spawn_child_creates_actor() {
  let child_slot = ArcShared::new(ActorRuntimeMutex::new(None));
  let log = ArcShared::new(ActorRuntimeMutex::new(Vec::new()));

  let props = Props::from_fn({
    let slot = child_slot.clone();
    let log = log.clone();
    move || ChildSpawner::new(slot.clone(), log.clone())
  });

  let system = ActorSystem::new(&props).expect("system");
  let guardian = system.user_guardian_ref();

  guardian.tell(AnyMessage::new(Start)).expect("start");

  let child_ref = child_slot.lock().clone().expect("child ref");
  child_ref.tell(AnyMessage::new(7_u32)).expect("child");

  assert_eq!(log.lock().clone(), vec![7_u32]);
}

#[derive(Clone)]
struct Ping(u32);

struct Pong(u32);

struct Responder;

impl Actor for Responder {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(ping) = message.downcast_ref::<Ping>() {
      ctx
        .reply(AnyMessage::new(Pong(ping.0)))
        .map_err(|_| ActorError::recoverable(ActorErrorReason::new("reply failed")))?;
    }
    Ok(())
  }
}

struct Probe {
  log: ArcShared<ActorRuntimeMutex<Vec<u32>>>,
}

impl Probe {
  fn new(log: ArcShared<ActorRuntimeMutex<Vec<u32>>>) -> Self {
    Self { log }
  }
}

impl Actor for Probe {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(pong) = message.downcast_ref::<Pong>() {
      self.log.lock().push(pong.0);
    }
    Ok(())
  }
}

struct ReplyGuardian {
  responder_slot: ArcShared<ActorRuntimeMutex<Option<ChildRef>>>,
  probe_slot:     ArcShared<ActorRuntimeMutex<Option<ChildRef>>>,
  log:            ArcShared<ActorRuntimeMutex<Vec<u32>>>,
}

impl ReplyGuardian {
  fn new(
    responder_slot: ArcShared<ActorRuntimeMutex<Option<ChildRef>>>,
    probe_slot: ArcShared<ActorRuntimeMutex<Option<ChildRef>>>,
    log: ArcShared<ActorRuntimeMutex<Vec<u32>>>,
  ) -> Self {
    Self { responder_slot, probe_slot, log }
  }
}

impl Actor for ReplyGuardian {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_some() && self.responder_slot.lock().is_none() {
      let probe_props = Props::from_fn({
        let log = self.log.clone();
        move || Probe::new(log.clone())
      });
      let probe =
        ctx.spawn_child(&probe_props).map_err(|_| ActorError::recoverable(ActorErrorReason::new("spawn failed")))?;
      self.probe_slot.lock().replace(probe.clone());

      let responder_props = Props::from_fn(|| Responder);
      let responder = ctx
        .spawn_child(&responder_props)
        .map_err(|_| ActorError::recoverable(ActorErrorReason::new("spawn failed")))?;
      self.responder_slot.lock().replace(responder);
    }
    Ok(())
  }
}

struct AskGuardian {
  responder_slot: ArcShared<ActorRuntimeMutex<Option<ChildRef>>>,
}

impl AskGuardian {
  fn new(responder_slot: ArcShared<ActorRuntimeMutex<Option<ChildRef>>>) -> Self {
    Self { responder_slot }
  }
}

impl Actor for AskGuardian {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_some() && self.responder_slot.lock().is_none() {
      let responder_props = Props::from_fn(|| Responder);
      let responder = ctx
        .spawn_child(&responder_props)
        .map_err(|_| ActorError::recoverable(ActorErrorReason::new("spawn failed")))?;
      self.responder_slot.lock().replace(responder);
    }
    Ok(())
  }
}

#[test]
fn reply_to_dispatches_response() {
  let responder_slot = ArcShared::new(ActorRuntimeMutex::new(None));
  let probe_slot = ArcShared::new(ActorRuntimeMutex::new(None));
  let log = ArcShared::new(ActorRuntimeMutex::new(Vec::new()));

  let props = Props::from_fn({
    let responder_slot = responder_slot.clone();
    let probe_slot = probe_slot.clone();
    let log = log.clone();
    move || ReplyGuardian::new(responder_slot.clone(), probe_slot.clone(), log.clone())
  });

  let system = ActorSystem::new(&props).expect("system");
  let guardian = system.user_guardian_ref();
  guardian.tell(AnyMessage::new(Start)).expect("boot");

  let responder = responder_slot.lock().clone().expect("responder");
  let probe = probe_slot.lock().clone().expect("probe");

  let message = AnyMessage::new(Ping(42)).with_reply_to(probe.actor_ref().clone());
  responder.tell(message).expect("send ping");

  assert_eq!(log.lock().clone(), vec![42_u32]);
}

#[test]
fn ask_registers_future_in_system() {
  let responder_slot = ArcShared::new(ActorRuntimeMutex::new(None));

  let props = Props::from_fn({
    let responder_slot = responder_slot.clone();
    move || AskGuardian::new(responder_slot.clone())
  });

  let system = ActorSystem::new(&props).expect("system");
  let guardian = system.user_guardian_ref();
  guardian.tell(AnyMessage::new(Start)).expect("boot");

  let responder = responder_slot.lock().clone().expect("responder");
  let response = responder.ask(AnyMessage::new(Ping(99))).expect("ask");

  let mut ready = system.drain_ready_ask_futures();
  assert_eq!(ready.len(), 1);
  let future = ready.pop().unwrap();
  let message = future.try_take().expect("ask result");
  let borrowed = message.as_view();
  assert_eq!(borrowed.downcast_ref::<Pong>().map(|p| p.0), Some(99));
  assert!(response.future().try_take().is_none());
}

struct StopChildActor {
  flag: ArcShared<ActorRuntimeMutex<bool>>,
}

impl StopChildActor {
  fn new(flag: ArcShared<ActorRuntimeMutex<bool>>) -> Self {
    Self { flag }
  }
}

impl Actor for StopChildActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }

  fn post_stop(&mut self, _ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    *self.flag.lock() = true;
    Ok(())
  }
}

struct StopSelfParent {
  child_pid:   ArcShared<ActorRuntimeMutex<Option<Pid>>>,
  child_flag:  ArcShared<ActorRuntimeMutex<bool>>,
  parent_flag: ArcShared<ActorRuntimeMutex<bool>>,
}

impl StopSelfParent {
  fn new(
    child_pid: ArcShared<ActorRuntimeMutex<Option<Pid>>>,
    child_flag: ArcShared<ActorRuntimeMutex<bool>>,
    parent_flag: ArcShared<ActorRuntimeMutex<bool>>,
  ) -> Self {
    Self { child_pid, child_flag, parent_flag }
  }
}

impl Actor for StopSelfParent {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_some() && self.child_pid.lock().is_none() {
      let child_props = Props::from_fn({
        let child_flag = self.child_flag.clone();
        move || StopChildActor::new(child_flag.clone())
      });
      let child = ctx
        .spawn_child(&child_props)
        .map_err(|_| ActorError::recoverable(ActorErrorReason::new("spawn child failed")))?;
      self.child_pid.lock().replace(child.pid());
      ctx.stop_self().map_err(|_| ActorError::recoverable(ActorErrorReason::new("stop self failed")))?;
    }
    Ok(())
  }

  fn post_stop(&mut self, _ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    *self.parent_flag.lock() = true;
    Ok(())
  }
}

#[test]
fn stop_self_propagates_to_children() {
  let child_pid = ArcShared::new(ActorRuntimeMutex::new(None));
  let child_stopped = ArcShared::new(ActorRuntimeMutex::new(false));
  let parent_stopped = ArcShared::new(ActorRuntimeMutex::new(false));

  let props = Props::from_fn({
    let child_pid = child_pid.clone();
    let child_stopped = child_stopped.clone();
    let parent_stopped = parent_stopped.clone();
    move || StopSelfParent::new(child_pid.clone(), child_stopped.clone(), parent_stopped.clone())
  });

  let system = ActorSystem::new(&props).expect("system");
  let guardian = system.user_guardian_ref();
  let guardian_pid = guardian.pid();
  guardian.tell(AnyMessage::new(Start)).expect("start");

  assert!(*parent_stopped.lock(), "parent post_stop should run");

  if let Some(child_pid) = *child_pid.lock() {
    assert!(*child_stopped.lock(), "child post_stop should run");
    assert!(system.actor_ref(child_pid).is_none(), "child should be removed from system");
  }

  assert!(system.actor_ref(guardian_pid).is_none(), "guardian should be removed after stop");
}
