use fraktor_utils_rs::core::sync::SharedAccess;

use crate::core::{
  kernel::actor::{ActorContext, error::ActorError, messaging::AnyMessage},
  typed::{
    TypedProps,
    actor::TypedActorContext,
    behavior::{Behavior, BehaviorDirective},
    dsl::{Behaviors, receive::Receive},
    message_and_signals::BehaviorSignal,
  },
};

// --- helpers ---------------------------------------------------------------

fn make_typed_ctx() -> (crate::core::kernel::system::ActorSystem, crate::core::kernel::actor::Pid) {
  let system = crate::core::kernel::system::ActorSystem::new_empty();
  let pid = system.allocate_pid();
  (system, pid)
}

// --- Behaviors::receive returns Receive<M> ---------------------------------

#[test]
fn behaviors_receive_returns_receive_type() {
  // Given: a message handler closure
  let handler =
    |_ctx: &mut TypedActorContext<'_, u32>, _msg: &u32| -> Result<Behavior<u32>, ActorError> { Ok(Behaviors::same()) };

  // When: Behaviors::receive is called
  let receive: Receive<u32> = Behaviors::receive(handler);

  // Then: a Receive<u32> is obtained (type-level verification)
  let _: Receive<u32> = receive;
}

// --- Receive::receive_signal chains into Behavior<M> -----------------------

#[test]
fn receive_signal_chains_into_behavior() {
  // Given: a Receive<u32> from Behaviors::receive
  let receive: Receive<u32> = Behaviors::receive(|_ctx, _msg| Ok(Behaviors::same()));

  // When: receive_signal is called with a signal handler
  let behavior: Behavior<u32> = receive.receive_signal(|_ctx, _signal| Ok(Behaviors::same()));

  // Then: the result is a Behavior<u32> with both message and signal handlers
  assert!(behavior.has_signal_handler(), "chained behavior should have a signal handler");
}

// --- Receive<M> converts to Behavior<M> via From --------------------------

#[test]
fn receive_converts_to_behavior_via_from() {
  // Given: a Receive<u32>
  let receive: Receive<u32> = Behaviors::receive(|_ctx, _msg| Ok(Behaviors::same()));

  // When: converted to Behavior<M> via Into
  let behavior: Behavior<u32> = receive.into();

  // Then: signal handler は付与されていない
  assert!(!behavior.has_signal_handler());
}

// --- Receive message handler is invoked correctly --------------------------

#[test]
fn receive_message_handler_is_invoked() {
  // Given: a Receive<u32> that echoes the message
  let receive =
    Behaviors::receive(|_ctx, msg: &u32| if *msg == 42 { Ok(Behaviors::stopped()) } else { Ok(Behaviors::same()) });

  // When: converted to Behavior and message is handled
  let mut behavior: Behavior<u32> = receive.into();
  let (system, pid) = make_typed_ctx();
  let mut context = ActorContext::new(&system, pid);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  let next = behavior.handle_message(&mut typed_ctx, &42).expect("message 42 should produce a next behavior");

  // Then: the handler returns Stopped for message 42
  assert!(matches!(next.directive(), BehaviorDirective::Stopped));
}

// --- Receive with chained signal handler handles both messages and signals --

#[test]
fn receive_with_signal_handles_both() {
  // Given: a Receive<u32> with both message and signal handlers
  let behavior: Behavior<u32> =
    Behaviors::receive(|_ctx, _msg: &u32| Ok(Behaviors::same())).receive_signal(|_ctx, signal| match signal {
      | BehaviorSignal::Stopped => Ok(Behaviors::stopped()),
      | _ => Ok(Behaviors::same()),
    });

  // Then: the behavior has both handlers
  assert!(behavior.has_signal_handler(), "behavior should have signal handler from chain");
}

// --- Behaviors::receive does not break existing receive_message -------------

#[test]
fn receive_message_still_works_independently() {
  // Given: an existing Behaviors::receive_message call
  let behavior: Behavior<u32> = Behaviors::receive_message(|_ctx, _msg| Ok(Behaviors::same()));

  // When: used directly as a Behavior (no Receive intermediate)
  let (system, pid) = make_typed_ctx();
  let mut context = ActorContext::new(&system, pid);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);
  let mut b = behavior;
  let result = b.handle_message(&mut typed_ctx, &99u32);

  // Then: same ビヘイビアを返す
  let next = result.expect("receive_message handler should return a next behavior");
  assert!(matches!(next.directive(), BehaviorDirective::Same));
}

#[test]
fn receive_can_be_used_directly_in_typed_props_factory() {
  // Given: a typed props built from Behaviors::receive
  let props = TypedProps::<u32>::from_behavior_factory(|| Behaviors::receive(|_ctx, _msg| Ok(Behaviors::same())));

  // When: invoking the stored factory through the untyped props layer
  let system = crate::core::kernel::system::ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut actor = props.to_untyped().factory().with_write(|factory| factory.create());
  let mut context = ActorContext::new(&system, pid);

  // Then: the produced actor can process a message successfully
  let result = actor.receive(&mut context, AnyMessage::new(7_u32).as_view());
  assert!(result.is_ok(), "typed props factory should produce a runnable actor");
}
