//! Subscribe command for classic FSM transition callbacks.

use crate::actor::actor_ref::ActorRef;

/// Message requesting transition callback subscription for an FSM actor.
#[derive(Clone)]
pub struct FsmSubscribeTransitionCallback {
  actor_ref: ActorRef,
}

impl FsmSubscribeTransitionCallback {
  /// Creates a subscribe-transition-callback command.
  #[must_use]
  pub const fn new(actor_ref: ActorRef) -> Self {
    Self { actor_ref }
  }

  /// Returns the actor that should receive transition notifications.
  #[must_use]
  pub const fn actor_ref(&self) -> &ActorRef {
    &self.actor_ref
  }
}
