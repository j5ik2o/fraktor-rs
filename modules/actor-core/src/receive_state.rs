/// Tracks the current receive behavior stack for an actor.
#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct ReceiveState {
  depth: usize,
}

impl ReceiveState {
  /// Creates a new state instance.
  #[must_use]
  pub const fn new() -> Self {
    Self { depth: 0 }
  }

  /// Pushes a new behavior onto the stack.
  pub fn push(&mut self) {
    self.depth += 1;
  }

  /// Pops the current behavior if present.
  pub fn pop(&mut self) {
    self.depth = self.depth.saturating_sub(1);
  }

  /// Returns the current stack depth.
  #[must_use]
  pub const fn depth(&self) -> usize {
    self.depth
  }
}
