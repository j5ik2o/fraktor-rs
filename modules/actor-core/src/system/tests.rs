use alloc::{vec, vec::Vec};

use cellactor_utils_core_rs::sync::{ArcShared, sync_mutex_like::SpinSyncMutex};

use super::ActorSystem;
use crate::{
  actor::Actor,
  actor_context::ActorContext,
  actor_error::{ActorError, ActorErrorReason},
  actor_ref::ActorRef,
  any_message::{AnyMessage, AnyOwnedMessage},
  props::Props,
};

struct Start;

struct GuardianLogger {
  log: ArcShared<SpinSyncMutex<Vec<&'static str>>>,
}

impl GuardianLogger {
  fn new(log: ArcShared<SpinSyncMutex<Vec<&'static str>>>) -> Self {
    Self { log }
  }
}

impl Actor for GuardianLogger {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, message: AnyMessage<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_some() {
      self.log.lock().push("start");
    }
    Ok(())
  }
}

#[test]
fn guardian_processes_message() {
  let log = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let props = Props::from_fn({
    let log = log.clone();
    move || GuardianLogger::new(log.clone())
  });
  let system = ActorSystem::new(&props).expect("create system");

  system.user_guardian_ref().tell(AnyOwnedMessage::new(Start)).expect("send");

  assert_eq!(log.lock().clone(), vec!["start"]);
}

struct ChildRecorder {
  log: ArcShared<SpinSyncMutex<Vec<u32>>>,
}

impl ChildRecorder {
  fn new(log: ArcShared<SpinSyncMutex<Vec<u32>>>) -> Self {
    Self { log }
  }
}

impl Actor for ChildRecorder {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, message: AnyMessage<'_>) -> Result<(), ActorError> {
    if let Some(value) = message.downcast_ref::<u32>() {
      self.log.lock().push(*value);
    }
    Ok(())
  }
}

struct ChildSpawner {
  child_slot: ArcShared<SpinSyncMutex<Option<ActorRef>>>,
  log:        ArcShared<SpinSyncMutex<Vec<u32>>>,
}

impl ChildSpawner {
  fn new(child_slot: ArcShared<SpinSyncMutex<Option<ActorRef>>>, log: ArcShared<SpinSyncMutex<Vec<u32>>>) -> Self {
    Self { child_slot, log }
  }
}

impl Actor for ChildSpawner {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessage<'_>) -> Result<(), ActorError> {
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
  let child_slot = ArcShared::new(SpinSyncMutex::new(None));
  let log = ArcShared::new(SpinSyncMutex::new(Vec::new()));

  let props = Props::from_fn({
    let slot = child_slot.clone();
    let log = log.clone();
    move || ChildSpawner::new(slot.clone(), log.clone())
  });

  let system = ActorSystem::new(&props).expect("system");
  let guardian = system.user_guardian_ref();

  guardian.tell(AnyOwnedMessage::new(Start)).expect("start");

  let child_ref = child_slot.lock().clone().expect("child ref");
  child_ref.tell(AnyOwnedMessage::new(7_u32)).expect("child");

  assert_eq!(log.lock().clone(), vec![7_u32]);
}

#[derive(Clone)]
struct Ping(u32);

struct Pong(u32);

struct Responder;

impl Actor for Responder {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessage<'_>) -> Result<(), ActorError> {
    if let Some(ping) = message.downcast_ref::<Ping>() {
      ctx
        .reply(AnyOwnedMessage::new(Pong(ping.0)))
        .map_err(|_| ActorError::recoverable(ActorErrorReason::new("reply failed")))?;
    }
    Ok(())
  }
}

struct Probe {
  log: ArcShared<SpinSyncMutex<Vec<u32>>>,
}

impl Probe {
  fn new(log: ArcShared<SpinSyncMutex<Vec<u32>>>) -> Self {
    Self { log }
  }
}

impl Actor for Probe {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, message: AnyMessage<'_>) -> Result<(), ActorError> {
    if let Some(pong) = message.downcast_ref::<Pong>() {
      self.log.lock().push(pong.0);
    }
    Ok(())
  }
}

struct ReplyGuardian {
  responder_slot: ArcShared<SpinSyncMutex<Option<ActorRef>>>,
  probe_slot:     ArcShared<SpinSyncMutex<Option<ActorRef>>>,
  log:            ArcShared<SpinSyncMutex<Vec<u32>>>,
}

impl ReplyGuardian {
  fn new(
    responder_slot: ArcShared<SpinSyncMutex<Option<ActorRef>>>,
    probe_slot: ArcShared<SpinSyncMutex<Option<ActorRef>>>,
    log: ArcShared<SpinSyncMutex<Vec<u32>>>,
  ) -> Self {
    Self { responder_slot, probe_slot, log }
  }
}

impl Actor for ReplyGuardian {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessage<'_>) -> Result<(), ActorError> {
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

#[test]
fn reply_to_dispatches_response() {
  let responder_slot = ArcShared::new(SpinSyncMutex::new(None));
  let probe_slot = ArcShared::new(SpinSyncMutex::new(None));
  let log = ArcShared::new(SpinSyncMutex::new(Vec::new()));

  let props = Props::from_fn({
    let responder_slot = responder_slot.clone();
    let probe_slot = probe_slot.clone();
    let log = log.clone();
    move || ReplyGuardian::new(responder_slot.clone(), probe_slot.clone(), log.clone())
  });

  let system = ActorSystem::new(&props).expect("system");
  let guardian = system.user_guardian_ref();
  guardian.tell(AnyOwnedMessage::new(Start)).expect("boot");

  let responder = responder_slot.lock().clone().expect("responder");
  let probe = probe_slot.lock().clone().expect("probe");

  let message = AnyOwnedMessage::new(Ping(42)).with_reply_to(probe);
  responder.tell(message).expect("send ping");

  assert_eq!(log.lock().clone(), vec![42_u32]);
}
