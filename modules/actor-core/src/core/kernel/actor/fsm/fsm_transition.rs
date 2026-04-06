//! Transition directives returned by classic FSM handlers.

use super::FsmReason;

/// Transition directive returned from a classic FSM state handler.
pub struct FsmTransition<State, Data> {
  next_state:  Option<State>,
  next_data:   Option<Data>,
  stop_reason: Option<FsmReason>,
  handled:     bool,
}

impl<State, Data> FsmTransition<State, Data> {
  /// Keeps the current state and data.
  #[must_use]
  pub const fn stay() -> Self {
    Self { next_state: None, next_data: None, stop_reason: None, handled: true }
  }

  /// Moves to the provided next state.
  #[must_use]
  pub const fn goto(next_state: State) -> Self {
    Self { next_state: Some(next_state), next_data: None, stop_reason: None, handled: true }
  }

  /// Stops the FSM with the provided reason.
  #[must_use]
  pub const fn stop(reason: FsmReason) -> Self {
    Self { next_state: None, next_data: None, stop_reason: Some(reason), handled: true }
  }

  /// Marks the message as not handled by the current FSM state.
  #[must_use]
  pub const fn unhandled() -> Self {
    Self { next_state: None, next_data: None, stop_reason: None, handled: false }
  }

  /// Replaces the state data associated with the next state.
  #[must_use]
  pub fn using(mut self, data: Data) -> Self {
    self.next_data = Some(data);
    self
  }

  pub(crate) const fn handled(&self) -> bool {
    self.handled
  }

  pub(crate) fn into_parts(self) -> (Option<State>, Option<Data>, Option<FsmReason>) {
    (self.next_state, self.next_data, self.stop_reason)
  }
}
