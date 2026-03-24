use core::sync::atomic::{AtomicUsize, Ordering};

use fraktor_utils_rs::core::sync::ArcShared;

use super::ActorRef;
use crate::core::{
  actor::{
    Pid,
    actor_ref::{ActorRefSender, NullSender},
  },
  error::SendError,
  messaging::AnyMessage,
  system::ActorSystem,
};

struct RecordingSender {
  count: ArcShared<AtomicUsize>,
}

impl RecordingSender {
  fn new() -> (ArcShared<AtomicUsize>, Self) {
    let count = ArcShared::new(AtomicUsize::new(0));
    let sender = Self { count: count.clone() };
    (count, sender)
  }
}

impl ActorRefSender for RecordingSender {
  fn send(&mut self, _message: AnyMessage) -> Result<crate::core::actor::actor_ref::SendOutcome, SendError> {
    use core::sync::atomic::Ordering;
    self.count.fetch_add(1, Ordering::Relaxed);
    Ok(crate::core::actor::actor_ref::SendOutcome::Delivered)
  }
}

#[test]
fn null_sender_uses_fire_and_forget_tell_contract() {
  let null: ActorRef = ActorRef::null();
  let _: () = null.tell(AnyMessage::new(1_u32));
}

#[test]
fn new_actor_ref_forwards_messages() {
  let (count, sender) = RecordingSender::new();
  let actor: ActorRef = ActorRef::new(Pid::new(1, 0), sender);
  let _: () = actor.tell(AnyMessage::new(42_u32));
  assert_eq!(count.load(Ordering::Relaxed), 1);
}

#[test]
fn actor_ref_pid() {
  let pid = Pid::new(42, 1);
  let (_, sender) = RecordingSender::new();
  let actor: ActorRef = ActorRef::new(pid, sender);
  assert_eq!(actor.pid(), pid);
}

#[test]
fn actor_ref_clone() {
  let (count, sender) = RecordingSender::new();
  let actor1: ActorRef = ActorRef::new(Pid::new(1, 0), sender);
  let actor2 = actor1.clone();

  assert_eq!(actor1.pid(), actor2.pid());

  let _: () = actor1.tell(AnyMessage::new(1_u32));
  let _: () = actor2.tell(AnyMessage::new(2_u32));
  assert_eq!(count.load(Ordering::Relaxed), 2);
}

#[test]
fn actor_ref_with_system() {
  let (_, sender) = RecordingSender::new();
  let system = ActorSystem::new_empty().state();
  let pid = Pid::new(1, 0);
  let actor: ActorRef = ActorRef::with_system(pid, sender, &system);

  assert_eq!(actor.pid(), pid);
  let _ = actor;
}

#[test]
fn actor_ref_path_resolves_segments() {
  use crate::core::{
    actor::{Actor, ActorCell, ActorContext},
    messaging::AnyMessageView,
    props::Props,
  };

  struct PathActor;
  impl Actor for PathActor {
    fn receive(
      &mut self,
      _ctx: &mut ActorContext<'_>,
      _message: AnyMessageView<'_>,
    ) -> Result<(), crate::core::error::ActorError> {
      Ok(())
    }
  }

  let system = ActorSystem::new_empty().state();
  let root_pid = system.allocate_pid();
  let child_pid = system.allocate_pid();
  let props = Props::from_fn(|| PathActor);
  let root = ActorCell::create(system.clone(), root_pid, None, "root".into(), &props).expect("create actor cell");
  system.register_cell(root);
  let child =
    ActorCell::create(system.clone(), child_pid, Some(root_pid), "worker".into(), &props).expect("create actor cell");
  system.register_cell(child);

  use crate::core::actor::actor_ref::null_sender::NullSender;
  let actor: ActorRef = ActorRef::with_system(child_pid, NullSender, &system);
  assert_eq!(actor.path().expect("path").to_string(), "/user/worker");
}

#[test]
fn actor_ref_tell_with_system_records_error() {
  let system = ActorSystem::new_empty().state();
  let pid = Pid::new(1, 0);
  let actor: ActorRef = ActorRef::with_system(pid, NullSender, &system);

  let _: () = actor.tell(AnyMessage::new(42_u32));
  let deadletters = system.dead_letters();
  assert_eq!(deadletters.len(), 1);
}

#[test]
fn actor_ref_ask_still_returns_send_error_when_delivery_fails() {
  let actor: ActorRef = ActorRef::null();

  let error = match actor.ask(AnyMessage::new(42_u32)) {
    | Ok(_) => panic!("ask should fail"),
    | Err(error) => error,
  };
  assert!(matches!(error, SendError::Closed(_)));
}

#[test]
fn actor_ref_partial_eq() {
  let (_, sender1) = RecordingSender::new();
  let (_, sender2) = RecordingSender::new();
  let (_, sender3) = RecordingSender::new();
  let pid = Pid::new(1, 0);

  let actor1: ActorRef = ActorRef::new(pid, sender1);
  let actor2: ActorRef = ActorRef::new(pid, sender2);
  let actor3: ActorRef = ActorRef::new(Pid::new(2, 0), sender3);

  assert_eq!(actor1, actor2);
  assert_ne!(actor1, actor3);
}

#[test]
fn actor_ref_debug() {
  extern crate alloc;
  use alloc::format;

  let (_, sender) = RecordingSender::new();
  let pid = Pid::new(42, 1);
  let actor: ActorRef = ActorRef::new(pid, sender);

  let debug_str = format!("{:?}", actor);
  assert!(debug_str.contains("ActorRef"));
  assert!(debug_str.contains("pid"));
}

#[test]
fn actor_ref_hash() {
  let (_, sender1) = RecordingSender::new();
  let (_, sender2) = RecordingSender::new();
  let pid = Pid::new(1, 0);

  let actor1: ActorRef = ActorRef::new(pid, sender1);
  let actor2: ActorRef = ActorRef::new(pid, sender2);

  let _ = actor1;
  let _ = actor2;
}

#[test]
fn no_sender_is_equivalent_to_null() {
  let no_sender: ActorRef = ActorRef::no_sender();
  let null: ActorRef = ActorRef::null();
  assert_eq!(no_sender.pid(), null.pid());
  let _: () = no_sender.tell(AnyMessage::new(1_u32));
}

#[test]
fn actor_ref_poison_pill_without_system_uses_user_channel() {
  let (count, sender) = RecordingSender::new();
  let actor: ActorRef = ActorRef::new(Pid::new(10, 0), sender);
  let _: () = actor.poison_pill();
  assert_eq!(count.load(Ordering::Relaxed), 1);
}

#[test]
fn actor_ref_kill_without_system_uses_user_channel() {
  let (count, sender) = RecordingSender::new();
  let actor: ActorRef = ActorRef::new(Pid::new(11, 0), sender);
  let _: () = actor.kill();
  assert_eq!(count.load(Ordering::Relaxed), 1);
}

#[test]
fn actor_ref_poison_pill_with_system_enqueues_user_message() {
  use crate::core::{
    actor::{Actor, ActorCell, ActorContext},
    messaging::AnyMessageView,
    props::Props,
  };

  struct ProbeActor;
  impl Actor for ProbeActor {
    fn receive(
      &mut self,
      _ctx: &mut ActorContext<'_>,
      _message: AnyMessageView<'_>,
    ) -> Result<(), crate::core::error::ActorError> {
      Ok(())
    }
  }

  let system = ActorSystem::new_empty().state();
  let pid = system.allocate_pid();
  let props = Props::from_fn(|| ProbeActor);
  let cell = ActorCell::create(system.clone(), pid, None, "probe".into(), &props).expect("create actor cell");
  system.register_cell(cell.clone());

  let actor: ActorRef = cell.actor_ref();
  let _: () = actor.poison_pill();
  assert_eq!(system.dead_letters().len(), 0, "poison pill via user channel should not produce dead letters");
}
