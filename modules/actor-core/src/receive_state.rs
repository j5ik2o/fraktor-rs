//! Receive state machine handling `become`/`unbecome` transitions.

use alloc::vec::Vec;

/// Identifier referencing a behaviour handler stored elsewhere in the runtime.
pub type BehaviorId = u16;

/// Tracks the current receive behavior for an actor.
#[derive(Debug, Default)]
pub struct ReceiveState {
  current: BehaviorId,
  stack:   Vec<BehaviorId>,
}

impl ReceiveState {
  /// Creates a new state with the provided behaviour identifier.
  #[must_use]
  pub const fn new(initial: BehaviorId) -> Self {
    Self { current: initial, stack: Vec::new() }
  }

  /// Returns the currently active behaviour identifier.
  #[must_use]
  pub const fn current(&self) -> BehaviorId {
    self.current
  }

  /// Pushes the current behaviour onto the stack and switches to the supplied one.
  pub fn push_behavior(&mut self, next: BehaviorId) {
    self.stack.push(self.current);
    self.current = next;
  }

  /// Restores the previous behaviour if any.
  pub fn pop_behavior(&mut self) -> Option<BehaviorId> {
    if let Some(previous) = self.stack.pop() {
      self.current = previous;
      Some(previous)
    } else {
      None
    }
  }

  /// Clears the behaviour stack and sets a new active behaviour.
  pub fn reset(&mut self, behavior: BehaviorId) {
    self.stack.clear();
    self.current = behavior;
  }
}
