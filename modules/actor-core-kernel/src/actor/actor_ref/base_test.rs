use alloc::vec::Vec;
use core::{
  hash::{Hash, Hasher},
  sync::atomic::{AtomicUsize, Ordering},
};

use fraktor_utils_core_rs::sync::{ArcShared, SharedAccess};

use super::ActorRef;
use crate::{
  actor::{
    Pid,
    actor_path::ActorPathParser,
    actor_ref::{ActorRefSender, NullSender, SendOutcome},
    error::{ActorError, SendError},
    messaging::AnyMessage,
  },
  system::ActorSystem,
};

struct RecordingSender {
  count: ArcShared<AtomicUsize>,
}

#[derive(Default)]
struct RecordingHasher {
  bytes: Vec<u8>,
}

impl Hasher for RecordingHasher {
  fn finish(&self) -> u64 {
    0
  }

  fn write(&mut self, bytes: &[u8]) {
    self.bytes.extend_from_slice(bytes);
  }
}

fn hash_bytes(actor_ref: &ActorRef) -> Vec<u8> {
  let mut hasher = RecordingHasher::default();
  actor_ref.hash(&mut hasher);
  hasher.bytes
}

impl RecordingSender {
  fn new() -> (ArcShared<AtomicUsize>, Self) {
    let count = ArcShared::new(AtomicUsize::new(0));
    let sender = Self { count: count.clone() };
    (count, sender)
  }
}

impl ActorRefSender for RecordingSender {
  fn send(&mut self, _message: AnyMessage) -> Result<SendOutcome, SendError> {
    use core::sync::atomic::Ordering;
    self.count.fetch_add(1, Ordering::Relaxed);
    Ok(SendOutcome::Delivered)
  }
}

fn build_path_aware_actor_ref() -> (ActorRef, ActorSystem) {
  use crate::actor::{Actor, ActorCell, ActorContext, messaging::AnyMessageView, props::Props};

  struct PathActor;
  impl Actor for PathActor {
    fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
      Ok(())
    }
  }

  let system = ActorSystem::new_empty();
  let state = system.state();
  let root_pid = state.allocate_pid();
  let child_pid = state.allocate_pid();
  let props = Props::from_fn(|| PathActor);
  let root = ActorCell::create(state.clone(), root_pid, None, "root".into(), &props).expect("create root actor cell");
  state.register_cell(root);
  let child =
    ActorCell::create(state.clone(), child_pid, Some(root_pid), "worker".into(), &props).expect("create actor cell");
  state.register_cell(child.clone());

  (child.actor_ref(), system)
}

#[test]
fn null_sender_try_tell_returns_closed() {
  let mut null: ActorRef = ActorRef::null();
  assert!(matches!(null.try_tell(AnyMessage::new(1_u32)), Err(SendError::Closed(_))));
}

#[test]
fn new_actor_ref_forwards_messages() {
  let (count, sender) = RecordingSender::new();
  let mut actor: ActorRef = ActorRef::new_with_builtin_lock(Pid::new(1, 0), sender);
  assert!(actor.try_tell(AnyMessage::new(42_u32)).is_ok());
  assert_eq!(count.load(Ordering::Relaxed), 1);
}

#[test]
fn actor_ref_pid() {
  let pid = Pid::new(42, 1);
  let (_, sender) = RecordingSender::new();
  let actor: ActorRef = ActorRef::new_with_builtin_lock(pid, sender);
  assert_eq!(actor.pid(), pid);
}

#[test]
fn actor_ref_clone() {
  let (count, sender) = RecordingSender::new();
  let mut actor1: ActorRef = ActorRef::new_with_builtin_lock(Pid::new(1, 0), sender);
  let mut actor2 = actor1.clone();

  assert_eq!(actor1.pid(), actor2.pid());

  assert!(actor1.try_tell(AnyMessage::new(1_u32)).is_ok());
  assert!(actor2.try_tell(AnyMessage::new(2_u32)).is_ok());
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
  use crate::actor::{Actor, ActorCell, ActorContext, messaging::AnyMessageView, props::Props};

  struct PathActor;
  impl Actor for PathActor {
    fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
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

  use crate::actor::actor_ref::null_sender::NullSender;
  let actor: ActorRef = ActorRef::with_system(child_pid, NullSender, &system);
  assert_eq!(actor.path().expect("path").to_string(), "/user/worker");
}

#[test]
fn actor_ref_with_canonical_path_returns_explicit_remote_path() {
  let remote_path = ActorPathParser::parse("fraktor.tcp://remote-sys@10.0.0.1:2552/user/worker").expect("remote path");
  let actor = ActorRef::with_canonical_path(Pid::new(900, 0), NullSender, remote_path.clone());

  assert_eq!(actor.path().expect("path").to_canonical_uri(), remote_path.to_canonical_uri());
  assert_eq!(actor.canonical_path().expect("canonical path").to_canonical_uri(), remote_path.to_canonical_uri());
}

#[test]
fn actor_ref_with_canonical_path_equality_uses_explicit_path() {
  let first_path = ActorPathParser::parse("fraktor.tcp://remote-sys@10.0.0.1:2552/user/worker").expect("first path");
  let second_path = ActorPathParser::parse("fraktor.tcp://remote-sys@10.0.0.1:2552/user/other").expect("second path");
  let first = ActorRef::with_canonical_path(Pid::new(900, 0), NullSender, first_path.clone());
  let same = ActorRef::with_canonical_path(Pid::new(901, 0), NullSender, first_path);
  let different = ActorRef::with_canonical_path(Pid::new(900, 0), NullSender, second_path);

  assert_eq!(first, same);
  assert_ne!(first, different);
}

#[test]
fn actor_ref_equality_matches_system_and_pid_only_refs() {
  let (system_ref, _system) = build_path_aware_actor_ref();
  let pid_only_ref = ActorRef::new_with_builtin_lock(system_ref.pid(), NullSender);

  assert_eq!(system_ref, pid_only_ref);
  assert_eq!(pid_only_ref, system_ref);
}

#[test]
fn actor_ref_equality_separates_explicit_canonical_path_and_pid_only_refs() {
  let remote_path = ActorPathParser::parse("fraktor.tcp://remote-sys@10.0.0.1:2552/user/worker").expect("remote path");
  let path_based = ActorRef::with_canonical_path(Pid::new(900, 0), NullSender, remote_path);
  let pid_based = ActorRef::new_with_builtin_lock(path_based.pid(), NullSender);

  assert_ne!(path_based, pid_based);
  assert_ne!(pid_based, path_based);
}

#[test]
fn actor_ref_hash_separates_pid_and_path_domains() {
  let remote_path = ActorPathParser::parse("fraktor.tcp://remote-sys@10.0.0.1:2552/user/worker").expect("remote path");
  let path_based = ActorRef::with_canonical_path(Pid::new(1, 0), NullSender, remote_path);
  let pid_based = ActorRef::new_with_builtin_lock(Pid::new(1, 0), NullSender);

  let path_hash_bytes = hash_bytes(&path_based);
  let pid_hash_bytes = hash_bytes(&pid_based);

  assert_eq!(path_hash_bytes.first().copied(), Some(1));
  assert_eq!(pid_hash_bytes.first().copied(), Some(0));
  assert_ne!(path_hash_bytes, pid_hash_bytes);
}

#[test]
fn actor_ref_equality_and_hash_stay_stable_after_system_drop() {
  let (system_ref, system) = build_path_aware_actor_ref();
  let pid_only_ref = ActorRef::new_with_builtin_lock(system_ref.pid(), NullSender);
  let system_hash = hash_bytes(&system_ref);

  assert_eq!(system_ref, pid_only_ref);
  assert_eq!(hash_bytes(&system_ref), hash_bytes(&pid_only_ref));
  drop(system);

  assert_eq!(system_ref, pid_only_ref);
  assert_eq!(hash_bytes(&system_ref), system_hash);
}

#[test]
fn actor_ref_try_tell_with_system_records_error() {
  let system = ActorSystem::new_empty().state();
  let pid = Pid::new(1, 0);
  let mut actor: ActorRef = ActorRef::with_system(pid, NullSender, &system);

  assert!(matches!(actor.try_tell(AnyMessage::new(42_u32)), Err(SendError::Closed(_))));
  let deadletters = system.dead_letters();
  assert_eq!(deadletters.len(), 1);
}

#[test]
fn actor_ref_ask_completes_send_failed_when_delivery_fails() {
  let mut actor: ActorRef = ActorRef::null();

  let response = actor.ask(AnyMessage::new(42_u32));
  assert_ne!(response.sender().pid(), actor.pid(), "reply ref must not reuse target pid");
  let result = response.future().with_write(|future| future.try_take()).expect("future should be ready");
  assert!(matches!(result, Err(crate::actor::messaging::AskError::SendFailed(_))));
}

#[test]
fn actor_ref_ask_reply_sender_uses_distinct_pid_and_no_target_path() {
  use crate::actor::{Actor, ActorCell, ActorContext, messaging::AnyMessageView, props::Props};

  struct EchoActor;
  impl Actor for EchoActor {
    fn receive(&mut self, _ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
      if let Some(value) = message.downcast_ref::<u32>()
        && let Some(sender) = message.sender()
      {
        let mut sender = sender.clone();
        sender.tell(AnyMessage::new(*value));
      }
      Ok(())
    }
  }

  let system = ActorSystem::new_empty().state();
  let pid = system.allocate_pid();
  let props = Props::from_fn(|| EchoActor);
  let cell = ActorCell::create(system.clone(), pid, None, "ask-reply-probe".into(), &props).expect("create actor cell");
  system.register_cell(cell.clone());

  let mut actor = cell.actor_ref();
  let response = actor.ask(AnyMessage::new(7_u32));

  assert_ne!(response.sender().pid(), actor.pid(), "reply ref must not reuse target pid");
  assert!(response.sender().path().is_none(), "ephemeral reply ref must not resolve to target path");
}

#[test]
fn actor_ref_partial_eq() {
  let (_, sender1) = RecordingSender::new();
  let (_, sender2) = RecordingSender::new();
  let (_, sender3) = RecordingSender::new();
  let pid = Pid::new(1, 0);

  let actor1: ActorRef = ActorRef::new_with_builtin_lock(pid, sender1);
  let actor2: ActorRef = ActorRef::new_with_builtin_lock(pid, sender2);
  let actor3: ActorRef = ActorRef::new_with_builtin_lock(Pid::new(2, 0), sender3);

  assert_eq!(actor1, actor2);
  assert_ne!(actor1, actor3);
}

#[test]
fn actor_ref_debug() {
  extern crate alloc;
  use alloc::format;

  let (_, sender) = RecordingSender::new();
  let pid = Pid::new(42, 1);
  let actor: ActorRef = ActorRef::new_with_builtin_lock(pid, sender);

  let debug_str = format!("{:?}", actor);
  assert!(debug_str.contains("ActorRef"));
  assert!(debug_str.contains("pid"));
}

#[test]
fn actor_ref_hash() {
  let (_, sender1) = RecordingSender::new();
  let (_, sender2) = RecordingSender::new();
  let pid = Pid::new(1, 0);

  let actor1: ActorRef = ActorRef::new_with_builtin_lock(pid, sender1);
  let actor2: ActorRef = ActorRef::new_with_builtin_lock(pid, sender2);

  let _ = actor1;
  let _ = actor2;
}

#[test]
fn no_sender_try_tell_is_equivalent_to_null() {
  let mut no_sender: ActorRef = ActorRef::no_sender();
  let null: ActorRef = ActorRef::null();
  assert_eq!(no_sender.pid(), null.pid());
  assert!(matches!(no_sender.try_tell(AnyMessage::new(1_u32)), Err(SendError::Closed(_))));
}

#[test]
fn actor_ref_poison_pill_without_system_uses_user_channel() {
  let (count, sender) = RecordingSender::new();
  let mut actor: ActorRef = ActorRef::new_with_builtin_lock(Pid::new(10, 0), sender);
  actor.poison_pill();
  assert_eq!(count.load(Ordering::Relaxed), 1);
}

#[test]
fn actor_ref_kill_without_system_uses_user_channel() {
  let (count, sender) = RecordingSender::new();
  let mut actor: ActorRef = ActorRef::new_with_builtin_lock(Pid::new(11, 0), sender);
  actor.kill();
  assert_eq!(count.load(Ordering::Relaxed), 1);
}

#[test]
fn actor_ref_poison_pill_with_system_enqueues_user_message() {
  use crate::actor::{Actor, ActorCell, ActorContext, messaging::AnyMessageView, props::Props};

  struct ProbeActor;
  impl Actor for ProbeActor {
    fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
      Ok(())
    }
  }

  let system = ActorSystem::new_empty().state();
  let pid = system.allocate_pid();
  let props = Props::from_fn(|| ProbeActor);
  let cell = ActorCell::create(system.clone(), pid, None, "probe".into(), &props).expect("create actor cell");
  system.register_cell(cell.clone());

  let mut actor: ActorRef = cell.actor_ref();
  actor.poison_pill();
  assert_eq!(system.dead_letters().len(), 0, "poison pill via user channel should not produce dead letters");
}
