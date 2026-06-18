//! Transition notification for classic FSM transition subscribers.

use crate::actor::actor_ref::ActorRef;

/// Message sent to subscribers when an FSM performs an explicit state transition.
#[derive(Clone)]
pub struct FsmStateTransition<State> {
  fsm_ref: ActorRef,
  from:    State,
  to:      State,
}

impl<State> FsmStateTransition<State> {
  /// Creates a state-transition notification.
  #[must_use]
  pub const fn new(fsm_ref: ActorRef, from: State, to: State) -> Self {
    Self { fsm_ref, from, to }
  }

  /// Returns the FSM actor reference.
  #[must_use]
  pub const fn fsm_ref(&self) -> &ActorRef {
    &self.fsm_ref
  }

  /// Returns the previous state.
  #[must_use]
  pub const fn from(&self) -> &State {
    &self.from
  }

  /// Returns the next state.
  #[must_use]
  pub const fn to(&self) -> &State {
    &self.to
  }
}
