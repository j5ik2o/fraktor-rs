//! Unsubscribe command for classic FSM transition callbacks.

use crate::actor::actor_ref::ActorRef;

/// Message requesting transition callback removal for an FSM actor.
#[derive(Clone)]
pub struct FsmUnsubscribeTransitionCallback {
  actor_ref: ActorRef,
}

impl FsmUnsubscribeTransitionCallback {
  /// Creates an unsubscribe-transition-callback command.
  #[must_use]
  pub const fn new(actor_ref: ActorRef) -> Self {
    Self { actor_ref }
  }

  /// Returns the actor that should stop receiving transition notifications.
  #[must_use]
  pub const fn actor_ref(&self) -> &ActorRef {
    &self.actor_ref
  }
}
