//! Timeout marker message used by classic FSM state timers.

/// Message delivered to a classic FSM when its state timeout expires.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FsmStateTimeout<State> {
  state:      State,
  generation: u64,
}

impl<State> FsmStateTimeout<State> {
  /// Creates a new state-timeout marker.
  #[must_use]
  pub const fn new(state: State, generation: u64) -> Self {
    Self { state, generation }
  }

  /// Returns the state for which this timeout was armed.
  #[must_use]
  pub const fn state(&self) -> &State {
    &self.state
  }

  /// Returns the generation token used to ignore stale timeout deliveries.
  #[must_use]
  pub const fn generation(&self) -> u64 {
    self.generation
  }
}
