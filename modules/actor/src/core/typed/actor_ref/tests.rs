use alloc::collections::BTreeSet;

use crate::core::{
  kernel::{
    actor::{
      Actor, ActorCell, ActorContext, Pid,
      actor_ref::{ActorRef, ActorRefSender, SendOutcome},
      error::{ActorError, SendError},
      messaging::{AnyMessage, AnyMessageView},
      props::Props,
    },
    system::ActorSystem,
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

struct NoOpActor;

impl Actor for NoOpActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

/// `path` returns `Some` when the actor is registered in the system.
#[test]
fn path_returns_some_with_system() {
  let system = ActorSystem::new_empty();
  let state = system.state();
  let pid = state.allocate_pid();
  let props = Props::from_fn(|| NoOpActor);
  let cell = ActorCell::create(state.clone(), pid, None, "test-path-actor".into(), &props).expect("create actor");
  state.register_cell(cell.clone());
  let actor_ref = ActorRef::with_system(pid, NoOpSender, &state);
  let typed_ref = TypedActorRef::<u32>::from_untyped(actor_ref);

  assert!(typed_ref.path().is_some(), "path should be Some when actor is registered");
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

/// `map` (existing) and `narrow` produce the same result from identical input.
#[test]
fn narrow_is_consistent_with_map() {
  let actor_ref = ActorRef::new(Pid::new(5, 0), NoOpSender);

  let via_map: TypedActorRef<u64> = TypedActorRef::<u32>::from_untyped(actor_ref.clone()).map();
  let via_narrow: TypedActorRef<u64> = TypedActorRef::<u32>::from_untyped(actor_ref).narrow();

  assert_eq!(via_map.pid(), via_narrow.pid(), "map and narrow should produce same pid");
}

#[test]
fn typed_actor_ref_equality_and_order_are_consistent_by_pid() {
  let left = TypedActorRef::<u32>::from_untyped(ActorRef::new(Pid::new(77, 1), NoOpSender));
  let right = TypedActorRef::<u32>::from_untyped(ActorRef::null().with_pid(Pid::new(77, 1)));

  assert_eq!(left, right);
  assert_eq!(left.cmp(&right), core::cmp::Ordering::Equal);

  let set = BTreeSet::from([left, right]);
  assert_eq!(set.len(), 1, "BTreeSet dedup should match PartialEq semantics");
}
