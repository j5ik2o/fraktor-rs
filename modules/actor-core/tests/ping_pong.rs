#![cfg(feature = "std")]

extern crate alloc;

use alloc::{string::String, vec::Vec};

use cellactor_actor_core_rs::{
  Actor, ActorContext, ActorError, ActorSystem, AnyMessage, AnyMessageView, ChildRef, EnqueueOutcome, Mailbox,
  MailboxOverflowStrategy, MailboxPolicy, Props, SendError, SpawnError,
};
use cellactor_utils_core_rs::sync::{ArcShared, sync_mutex_like::SpinSyncMutex};

struct Start;
struct Deliver(u32);
struct RecordingChild {
  log: ArcShared<SpinSyncMutex<Vec<u32>>>,
}

impl RecordingChild {
  fn new(log: ArcShared<SpinSyncMutex<Vec<u32>>>) -> Self {
    Self { log }
  }
}

impl Actor for RecordingChild {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(deliver) = message.downcast_ref::<Deliver>() {
      self.log.lock().push(deliver.0);
    }
    Ok(())
  }
}

struct RecordingGuardian {
  child_log: ArcShared<SpinSyncMutex<Vec<u32>>>,
  child_ref: ArcShared<SpinSyncMutex<Option<ChildRef>>>,
}

impl RecordingGuardian {
  fn new(child_log: ArcShared<SpinSyncMutex<Vec<u32>>>, child_ref: ArcShared<SpinSyncMutex<Option<ChildRef>>>) -> Self {
    Self { child_log, child_ref }
  }
}

impl Actor for RecordingGuardian {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_some() {
      let log = self.child_log.clone();
      let child = ctx
        .spawn_child(&Props::from_fn(move || RecordingChild::new(log.clone())))
        .map_err(|_| ActorError::recoverable("spawn failed"))?;
      self.child_ref.lock().replace(child.clone());
      child.tell(AnyMessage::new(Deliver(99))).map_err(|_| ActorError::recoverable("send failed"))?;
    }
    Ok(())
  }
}

struct SilentActor;

impl Actor for SilentActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

struct NamingGuardian {
  conflict: ArcShared<SpinSyncMutex<bool>>,
  spawned:  ArcShared<SpinSyncMutex<Vec<u64>>>,
}

impl NamingGuardian {
  fn new(conflict: ArcShared<SpinSyncMutex<bool>>, spawned: ArcShared<SpinSyncMutex<Vec<u64>>>) -> Self {
    Self { conflict, spawned }
  }
}

impl Actor for NamingGuardian {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_some() {
      let _ = ctx
        .spawn_child(&Props::from_fn(|| SilentActor).with_name("worker"))
        .map(|actor| self.spawned.lock().push(actor.pid().value()));

      let duplicate = ctx.spawn_child(&Props::from_fn(|| SilentActor).with_name("worker"));
      let conflict_detected = matches!(duplicate, Err(SpawnError::NameConflict(_)));
      *self.conflict.lock() = conflict_detected;

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
  let log = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let child_ref = ArcShared::new(SpinSyncMutex::new(None));
  let props = Props::from_fn({
    let log = log.clone();
    let child_ref = child_ref.clone();
    move || RecordingGuardian::new(log.clone(), child_ref.clone())
  });
  let system = ActorSystem::new(props).expect("system");

  system.user_guardian_ref().tell(AnyMessage::new(Start)).expect("start message");

  let entries = log.lock().clone();
  assert_eq!(entries, vec![99]);
  assert!(child_ref.lock().is_some());
}

#[test]
fn tell_respects_mailbox_backpressure() {
  let mailbox = Mailbox::new(MailboxPolicy::bounded(
    core::num::NonZeroUsize::new(1).unwrap(),
    MailboxOverflowStrategy::DropNewest,
    None,
  ));

  assert!(matches!(mailbox.enqueue_user(AnyMessage::new(String::from("first"))), Ok(EnqueueOutcome::Enqueued)));
  let result = mailbox.enqueue_user(AnyMessage::new(String::from("second")));
  assert!(matches!(result, Err(SendError::Full(_))));
}

#[test]
fn auto_naming_and_duplicate_detection() {
  let conflict = ArcShared::new(SpinSyncMutex::new(false));
  let spawned = ArcShared::new(SpinSyncMutex::new(Vec::new()));

  let props = Props::from_fn({
    let conflict = conflict.clone();
    let spawned = spawned.clone();
    move || NamingGuardian::new(conflict.clone(), spawned.clone())
  });

  let system = ActorSystem::new(props).expect("system");
  system.user_guardian_ref().tell(AnyMessage::new(Start)).expect("start");

  assert!(*conflict.lock(), "expected name conflict for duplicate spawn");
  let ids = spawned.lock().clone();
  assert_eq!(ids.len(), 3);
  let mut unique = ids.clone();
  unique.sort_unstable();
  unique.dedup();
  assert_eq!(unique.len(), ids.len(), "pids should be unique");
}
