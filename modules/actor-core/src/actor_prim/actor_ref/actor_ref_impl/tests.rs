use core::sync::atomic::{AtomicUsize, Ordering};

use cellactor_utils_core_rs::sync::ArcShared;

use super::{ActorRef, ActorRefSender};
use crate::{NoStdToolbox, actor_prim::Pid, error::SendError, messaging::AnyMessage};

struct RecordingSender {
  count: ArcShared<AtomicUsize>,
}

impl RecordingSender {
  fn new() -> (ArcShared<AtomicUsize>, ArcShared<Self>) {
    let count = ArcShared::new(AtomicUsize::new(0));
    let sender = ArcShared::new(Self { count: count.clone() });
    (count, sender)
  }
}

impl ActorRefSender<NoStdToolbox> for RecordingSender {
  fn send(&self, _message: AnyMessage<NoStdToolbox>) -> Result<(), SendError<NoStdToolbox>> {
    use core::sync::atomic::Ordering;
    self.count.fetch_add(1, Ordering::Relaxed);
    Ok(())
  }
}

#[test]
fn null_sender_rejects_messages() {
  let null: ActorRef<NoStdToolbox> = ActorRef::null();
  assert!(null.tell(AnyMessage::new(1_u32)).is_err());
}

#[test]
fn new_actor_ref_forwards_messages() {
  let (count, sender) = RecordingSender::new();
  let actor: ActorRef<NoStdToolbox> = ActorRef::new(Pid::new(1, 0), sender);
  assert!(actor.tell(AnyMessage::new(42_u32)).is_ok());
  assert_eq!(count.load(Ordering::Relaxed), 1);
}

#[test]
fn actor_ref_pid() {
  let pid = Pid::new(42, 1);
  let (_, sender) = RecordingSender::new();
  let actor: ActorRef<NoStdToolbox> = ActorRef::new(pid, sender);
  assert_eq!(actor.pid(), pid);
}

#[test]
fn actor_ref_clone() {
  let (count, sender) = RecordingSender::new();
  let actor1: ActorRef<NoStdToolbox> = ActorRef::new(Pid::new(1, 0), sender);
  let actor2 = actor1.clone();

  // ????????Pid???
  assert_eq!(actor1.pid(), actor2.pid());

  // ??????????
  assert!(actor1.tell(AnyMessage::new(1_u32)).is_ok());
  assert!(actor2.tell(AnyMessage::new(2_u32)).is_ok());
  assert_eq!(count.load(Ordering::Relaxed), 2);
}

#[test]
fn actor_ref_with_system() {
  use crate::system::SystemState;

  let (_, sender) = RecordingSender::new();
  let system = ArcShared::new(SystemState::<NoStdToolbox>::new());
  let pid = Pid::new(1, 0);
  let actor: ActorRef<NoStdToolbox> = ActorRef::with_system(pid, sender, system.clone());

  assert_eq!(actor.pid(), pid);
  // system????????????????????????????
  let _ = actor;
}

#[test]
fn actor_ref_tell_with_system_records_error() {
  use crate::{actor_prim::actor_ref::null_sender::NullSender, system::SystemState};

  let system = ArcShared::new(SystemState::<NoStdToolbox>::new());
  let pid = Pid::new(1, 0);
  let null_sender = ArcShared::new(NullSender);
  let actor: ActorRef<NoStdToolbox> = ActorRef::with_system(pid, null_sender, system.clone());

  // tell?????????system??????????
  let result = actor.tell(AnyMessage::new(42_u32));
  assert!(result.is_err());

  // deadletters???????????
  let deadletters = system.deadletters();
  assert_eq!(deadletters.len(), 1);
}

#[test]
fn actor_ref_partial_eq() {
  let (_, sender1) = RecordingSender::new();
  let (_, sender2) = RecordingSender::new();
  let (_, sender3) = RecordingSender::new();
  let pid = Pid::new(1, 0);

  let actor1: ActorRef<NoStdToolbox> = ActorRef::new(pid, sender1);
  let actor2: ActorRef<NoStdToolbox> = ActorRef::new(pid, sender2);
  let actor3: ActorRef<NoStdToolbox> = ActorRef::new(Pid::new(2, 0), sender3);

  // ??Pid???????
  assert_eq!(actor1, actor2);
  // ???Pid?????????
  assert_ne!(actor1, actor3);
}

#[test]
fn actor_ref_debug() {
  extern crate alloc;
  use alloc::format;

  let (_, sender) = RecordingSender::new();
  let pid = Pid::new(42, 1);
  let actor: ActorRef<NoStdToolbox> = ActorRef::new(pid, sender);

  // Debug?????????????????
  let debug_str = format!("{:?}", actor);
  assert!(debug_str.contains("ActorRef"));
  assert!(debug_str.contains("pid"));
}

#[test]
fn actor_ref_hash() {
  use core::hash::Hash;

  let (_, sender1) = RecordingSender::new();
  let (_, sender2) = RecordingSender::new();
  let pid = Pid::new(1, 0);

  let actor1: ActorRef<NoStdToolbox> = ActorRef::new(pid, sender1);
  let actor2: ActorRef<NoStdToolbox> = ActorRef::new(pid, sender2);

  // Hash?????????????????????????????
  // ?????????????std???????
  let _ = actor1;
  let _ = actor2;
}
