#![cfg(not(target_os = "none"))]

use std::{thread, time::Duration, vec::Vec};

use fraktor_actor_core_rs::{
  NoStdToolbox,
  actor_prim::{Actor, ActorContextGeneric, ChildRef},
  error::{ActorError, SendError},
  mailbox::{Mailbox, MailboxOverflowStrategy, MailboxPolicy},
  messaging::{AnyMessage, AnyMessageViewGeneric},
  props::Props,
  spawn::SpawnError,
  system::ActorSystem,
};
use fraktor_utils_core_rs::core::sync::{ArcShared, NoStdMutex};

struct Start;
struct Deliver(u32);

struct RecordingChild {
  log: ArcShared<NoStdMutex<Vec<u32>>>,
}

impl RecordingChild {
  fn new(log: ArcShared<NoStdMutex<Vec<u32>>>) -> Self {
    Self { log }
  }
}

impl Actor for RecordingChild {
  fn receive(
    &mut self,
    _ctx: &mut ActorContextGeneric<'_, NoStdToolbox>,
    message: AnyMessageViewGeneric<'_, NoStdToolbox>,
  ) -> Result<(), ActorError> {
    if let Some(deliver) = message.downcast_ref::<Deliver>() {
      self.log.lock().push(deliver.0);
    }
    Ok(())
  }
}

struct RecordingGuardian {
  log:        ArcShared<NoStdMutex<Vec<u32>>>,
  child_slot: ArcShared<NoStdMutex<Option<ChildRef>>>,
}

impl RecordingGuardian {
  fn new(log: ArcShared<NoStdMutex<Vec<u32>>>, child_slot: ArcShared<NoStdMutex<Option<ChildRef>>>) -> Self {
    Self { log, child_slot }
  }
}

impl Actor for RecordingGuardian {
  fn receive(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, NoStdToolbox>,
    message: AnyMessageViewGeneric<'_, NoStdToolbox>,
  ) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_some() {
      let log = self.log.clone();
      let child = ctx
        .spawn_child(&Props::from_fn(move || RecordingChild::new(log.clone())))
        .map_err(|_| ActorError::recoverable("spawn failed"))?;
      self.child_slot.lock().replace(child.clone());
      child.tell(AnyMessage::new(Deliver(99))).map_err(|_| ActorError::recoverable("send failed"))?;
    }
    Ok(())
  }
}

struct SilentActor;

impl Actor for SilentActor {
  fn receive(
    &mut self,
    _ctx: &mut ActorContextGeneric<'_, NoStdToolbox>,
    _message: AnyMessageViewGeneric<'_, NoStdToolbox>,
  ) -> Result<(), ActorError> {
    Ok(())
  }
}

struct NamingGuardian {
  conflict: ArcShared<NoStdMutex<bool>>,
  spawned:  ArcShared<NoStdMutex<Vec<u64>>>,
}

impl NamingGuardian {
  fn new(conflict: ArcShared<NoStdMutex<bool>>, spawned: ArcShared<NoStdMutex<Vec<u64>>>) -> Self {
    Self { conflict, spawned }
  }
}

impl Actor for NamingGuardian {
  fn receive(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, NoStdToolbox>,
    message: AnyMessageViewGeneric<'_, NoStdToolbox>,
  ) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_some() {
      let _ = ctx
        .spawn_child(&Props::from_fn(|| SilentActor).with_name("worker"))
        .map(|actor| self.spawned.lock().push(actor.pid().value()));

      let duplicate = ctx.spawn_child(&Props::from_fn(|| SilentActor).with_name("worker"));
      *self.conflict.lock() = matches!(duplicate, Err(SpawnError::NameConflict(_)));

      for _ in 0..2 {
        let actor =
          ctx.spawn_child(&Props::from_fn(|| SilentActor)).map_err(|_| ActorError::recoverable("spawn failed"))?;
        self.spawned.lock().push(actor.pid().value());
      }
    }
    Ok(())
  }
}

#[test]
fn spawn_and_tell_delivers_message() {
  let log = ArcShared::new(NoStdMutex::new(Vec::new()));
  let child_slot = ArcShared::new(NoStdMutex::new(None));
  let props = Props::from_fn({
    let log = log.clone();
    let child_slot = child_slot.clone();
    move || RecordingGuardian::new(log.clone(), child_slot.clone())
  });
  let system = ActorSystem::new(&props).expect("system");

  system.user_guardian_ref().tell(AnyMessage::new(Start)).expect("start");

  let dead_line = std::time::Instant::now() + Duration::from_millis(20);
  while log.lock().is_empty() && std::time::Instant::now() < dead_line {
    thread::yield_now();
  }

  assert_eq!(*log.lock(), vec![99]);
  assert!(child_slot.lock().is_some());
}

#[test]
fn tell_respects_mailbox_backpressure() {
  use core::num::NonZeroUsize;

  let mailbox: Mailbox =
    Mailbox::new(MailboxPolicy::bounded(NonZeroUsize::new(1).unwrap(), MailboxOverflowStrategy::DropNewest, None));

  assert!(mailbox.enqueue_user(AnyMessage::new("first")).is_ok());
  let result = mailbox.enqueue_user(AnyMessage::new("second"));
  assert!(matches!(result, Err(SendError::Full(_))));
}

#[test]
fn auto_naming_and_duplicate_detection() {
  let conflict = ArcShared::new(NoStdMutex::new(false));
  let spawned = ArcShared::new(NoStdMutex::new(Vec::new()));

  let props = Props::from_fn({
    let conflict = conflict.clone();
    let spawned = spawned.clone();
    move || NamingGuardian::new(conflict.clone(), spawned.clone())
  });

  let system = ActorSystem::new(&props).expect("system");
  system.user_guardian_ref().tell(AnyMessage::new(Start)).expect("start");

  let dead_line = std::time::Instant::now() + Duration::from_millis(20);
  while spawned.lock().len() < 3 && std::time::Instant::now() < dead_line {
    thread::yield_now();
  }

  assert!(*conflict.lock(), "expected name conflict for duplicate spawn");
  let ids = spawned.lock().clone();
  assert_eq!(ids.len(), 3);
  let mut unique = ids.clone();
  unique.sort_unstable();
  unique.dedup();
  assert_eq!(unique.len(), ids.len(), "pids should be unique");
}
