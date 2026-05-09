#![cfg(not(target_os = "none"))]

use std::{
  thread,
  time::{Duration, Instant},
  vec::Vec,
};

use fraktor_actor_adaptor_std_rs::tick_driver::TestTickDriver;
use fraktor_actor_core_kernel_rs::{
  actor::{
    Actor, ActorContext, ChildRef,
    error::ActorError,
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
    setup::ActorSystemConfig,
    spawn::SpawnError,
  },
  dispatch::mailbox::{Mailbox, MailboxOverflowStrategy, MailboxPolicy},
  system::ActorSystem,
};
use fraktor_utils_core_rs::sync::{ArcShared, SpinSyncMutex};

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
  log:        ArcShared<SpinSyncMutex<Vec<u32>>>,
  child_slot: ArcShared<SpinSyncMutex<Option<ChildRef>>>,
}

impl RecordingGuardian {
  fn new(log: ArcShared<SpinSyncMutex<Vec<u32>>>, child_slot: ArcShared<SpinSyncMutex<Option<ChildRef>>>) -> Self {
    Self { log, child_slot }
  }
}

impl Actor for RecordingGuardian {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_some() {
      let log = self.log.clone();
      let mut child = ctx
        .spawn_child(&Props::from_fn(move || RecordingChild::new(log.clone())))
        .map_err(|_| ActorError::recoverable("spawn failed"))?;
      self.child_slot.lock().replace(child.clone());
      child.tell(AnyMessage::new(Deliver(99)));
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
      let actor = ctx
        .spawn_child(&Props::from_fn(|| SilentActor).with_name("worker"))
        .map_err(|_| ActorError::recoverable("named worker spawn failed"))?;
      self.spawned.lock().push(actor.pid().value());

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
  let log = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let child_slot = ArcShared::new(SpinSyncMutex::new(None));
  let props = Props::from_fn({
    let log = log.clone();
    let child_slot = child_slot.clone();
    move || RecordingGuardian::new(log.clone(), child_slot.clone())
  });
  let system =
    ActorSystem::create_from_props(&props, ActorSystemConfig::new(TestTickDriver::default())).expect("system");

  system.user_guardian_ref().tell(AnyMessage::new(Start));

  let dead_line = Instant::now() + Duration::from_millis(20);
  while log.lock().is_empty() && Instant::now() < dead_line {
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

  // Pekko 互換: `BoundedMailbox.enqueue` は受理時も overflow 時も呼び出し元へ
  // 成功として見せ、overflow は内部で `deadLetters` へ転送される。`MailboxFull`
  // の dead-letter 記録元は mailbox 層だけなので、queue に空きがなくても
  // 呼び出し元は `Ok(())` を観測する。
  //
  // queue レベルの backpressure セマンティクス (capacity 上限、DropNewest の
  // 破棄挙動) は `bounded_message_queue` / mailbox `base` のユニットテストで
  // 検証されている。integration 呼び出し元が確認すべき契約は
  // "`enqueue_user` が overflow を原因として Err を返さない" ことだけ。
  assert!(mailbox.enqueue_user(AnyMessage::new("first")).is_ok());
  assert!(
    mailbox.enqueue_user(AnyMessage::new("second")).is_ok(),
    "DropNewest overflow must be reported as success (Pekko void-on-success)",
  );
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

  let system =
    ActorSystem::create_from_props(&props, ActorSystemConfig::new(TestTickDriver::default())).expect("system");
  system.user_guardian_ref().tell(AnyMessage::new(Start));

  let dead_line = Instant::now() + Duration::from_millis(20);
  while spawned.lock().len() < 3 && Instant::now() < dead_line {
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
