//! Current-state notification for classic FSM transition subscribers.

use crate::actor::actor_ref::ActorRef;

/// Message sent to a subscriber immediately after FSM transition subscription.
#[derive(Clone)]
pub struct FsmCurrentState<State> {
  fsm_ref: ActorRef,
  state:   State,
}

impl<State> FsmCurrentState<State> {
  /// Creates a current-state notification.
  #[must_use]
  pub const fn new(fsm_ref: ActorRef, state: State) -> Self {
    Self { fsm_ref, state }
  }

  /// Returns the FSM actor reference.
  #[must_use]
  pub const fn fsm_ref(&self) -> &ActorRef {
    &self.fsm_ref
  }

  /// Returns the current FSM state.
  #[must_use]
  pub const fn state(&self) -> &State {
    &self.state
  }
}
