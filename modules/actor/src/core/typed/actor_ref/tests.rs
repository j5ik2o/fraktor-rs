use crate::core::{
  kernel::actor::{
    Pid,
    actor_ref::{ActorRef, ActorRefSender, SendOutcome},
    error::SendError,
    messaging::AnyMessage,
  },
  typed::TypedActorRef,
};

struct NoOpSender;

impl ActorRefSender for NoOpSender {
  fn send(&mut self, _message: AnyMessage) -> Result<SendOutcome, SendError> {
    Ok(SendOutcome::Delivered)
  }
}

// --- Phase 1 タスク3: path / narrow / unsafe_upcast ---

/// `path` returns `None` when the actor reference has no system.
#[test]
fn path_returns_none_without_system() {
  let actor_ref = ActorRef::new(Pid::new(1, 0), NoOpSender);
  let typed_ref = TypedActorRef::<u32>::from_untyped(actor_ref);

  assert!(typed_ref.path().is_none(), "path should be None without system");
}

/// `narrow` converts the reference to a different message type.
#[test]
fn narrow_converts_message_type() {
  let actor_ref = ActorRef::new(Pid::new(2, 0), NoOpSender);
  let typed_ref = TypedActorRef::<u32>::from_untyped(actor_ref);
  let original_pid = typed_ref.pid();

  let narrowed: TypedActorRef<u16> = typed_ref.narrow();

  assert_eq!(narrowed.pid(), original_pid, "pid should be preserved after narrow");
}

/// `unsafe_upcast` converts the reference to a wider message type.
#[test]
fn unsafe_upcast_converts_message_type() {
  let actor_ref = ActorRef::new(Pid::new(3, 0), NoOpSender);
  let typed_ref = TypedActorRef::<u32>::from_untyped(actor_ref);
  let original_pid = typed_ref.pid();

  let upcasted: TypedActorRef<u64> = typed_ref.unsafe_upcast();

  assert_eq!(upcasted.pid(), original_pid, "pid should be preserved after unsafe_upcast");
}

/// `narrow` followed by `unsafe_upcast` round-trips without changing identity.
#[test]
fn narrow_and_unsafe_upcast_round_trip() {
  let actor_ref = ActorRef::new(Pid::new(4, 0), NoOpSender);
  let typed_ref = TypedActorRef::<u32>::from_untyped(actor_ref);
  let original_pid = typed_ref.pid();

  let narrowed: TypedActorRef<u16> = typed_ref.narrow();
  let restored: TypedActorRef<u32> = narrowed.unsafe_upcast();

  assert_eq!(restored.pid(), original_pid, "round-trip should preserve pid");
}

/// `map` (existing) and `narrow` produce the same result.
#[test]
fn narrow_is_consistent_with_map() {
  let actor_ref_1 = ActorRef::new(Pid::new(5, 0), NoOpSender);
  let actor_ref_2 = ActorRef::new(Pid::new(5, 0), NoOpSender);

  let via_map: TypedActorRef<u64> = TypedActorRef::<u32>::from_untyped(actor_ref_1).map();
  let via_narrow: TypedActorRef<u64> = TypedActorRef::<u32>::from_untyped(actor_ref_2).narrow();

  assert_eq!(via_map.pid(), via_narrow.pid(), "map and narrow should produce same pid");
}
