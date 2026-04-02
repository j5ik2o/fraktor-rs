use alloc::collections::BTreeSet;
use core::hint::spin_loop;

use crate::core::{
  kernel::{
    actor::{
      Actor, ActorCell, ActorContext, Pid,
      actor_ref::{ActorRef, ActorRefSender, SendOutcome},
      error::{ActorError, SendError},
      messaging::{AnyMessage, AnyMessageView, AskError},
      props::Props,
    },
    system::ActorSystem,
  },
  typed::{TypedActorRef, dsl::TypedAskError},
};

struct NoOpSender;

impl ActorRefSender for NoOpSender {
  fn send(&mut self, _message: AnyMessage) -> Result<SendOutcome, SendError> {
    Ok(SendOutcome::Delivered)
  }
}

struct AlternateNoOpSender;

impl ActorRefSender for AlternateNoOpSender {
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

#[derive(Clone)]
struct EchoRequest {
  value:    u32,
  reply_to: TypedActorRef<u32>,
}

struct EchoActor;

impl Actor for EchoActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(request) = message.downcast_ref::<EchoRequest>() {
      let mut reply_to = request.reply_to.clone();
      reply_to.tell(request.value);
    }
    Ok(())
  }
}

fn wait_until(mut condition: impl FnMut() -> bool) {
  for _ in 0..10_000 {
    if condition() {
      return;
    }
    spin_loop();
  }
  panic!("condition not met");
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
  let right = TypedActorRef::<u32>::from_untyped(ActorRef::new(Pid::new(77, 1), AlternateNoOpSender));

  assert_eq!(left, right);
  assert_eq!(left.cmp(&right), core::cmp::Ordering::Equal);

  let set = BTreeSet::from([left, right]);
  assert_eq!(set.len(), 1, "BTreeSet dedup should match PartialEq semantics");
}

#[test]
fn typed_actor_ref_ask_returns_typed_reply_and_registers_future() {
  let system = ActorSystem::new_empty();
  let state = system.state();
  let pid = state.allocate_pid();
  let props = Props::from_fn(|| EchoActor);
  let cell = ActorCell::create(state.clone(), pid, None, "typed-ask-echo".into(), &props).expect("create actor");
  state.register_cell(cell.clone());
  let mut typed_ref = TypedActorRef::<EchoRequest>::from_untyped(cell.actor_ref());

  let response = typed_ref.ask::<u32, _>(|reply_to| EchoRequest { value: 55, reply_to });
  let mut future = response.future().clone();
  wait_until(|| future.is_ready());

  assert_ne!(response.sender().pid(), typed_ref.pid(), "typed reply ref must not reuse target pid");
  assert!(response.sender().path().is_none(), "typed reply ref must not resolve to target actor path");
  assert_eq!(system.drain_ready_ask_futures().len(), 1, "typed ask should register future with system");
  assert_eq!(future.try_take().expect("ready").expect("ok"), 55);
}

#[test]
fn typed_actor_ref_ask_reports_send_failure() {
  let mut typed_ref = TypedActorRef::<EchoRequest>::from_untyped(ActorRef::null());

  let response = typed_ref.ask::<u32, _>(|reply_to| EchoRequest { value: 1, reply_to });
  let mut future = response.future().clone();
  wait_until(|| future.is_ready());

  assert!(matches!(future.try_take().expect("ready"), Err(TypedAskError::AskFailed(AskError::SendFailed(_)))));
}
