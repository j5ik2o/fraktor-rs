//! Become/unbecome state machine monitoring active behaviors.

mod handler;

use alloc::{vec, vec::Vec};

use cellactor_utils_core_rs::ArcShared;
pub use handler::ReceiveHandler;

/// Tracks the stack of active receive behaviors for an actor.
pub struct ReceiveState {
  stack: Vec<ArcShared<dyn ReceiveHandler>>,
}

impl ReceiveState {
  /// Creates a new receive state with the provided initial behavior.
  #[must_use]
  pub fn new(initial: ArcShared<dyn ReceiveHandler>) -> Self {
    Self { stack: vec![initial] }
  }

  /// Returns the current behavior at the top of the stack.
  #[must_use]
  pub fn current(&self) -> &ArcShared<dyn ReceiveHandler> {
    self.stack.last().expect("receive behavior stack is never empty")
  }

  /// Pushes a new behavior onto the stack.
  pub fn r#become(&mut self, behavior: ArcShared<dyn ReceiveHandler>) {
    self.stack.push(behavior);
  }

  /// Restores the previous behavior if one exists.
  #[must_use]
  pub fn unbecome(&mut self) -> Option<ArcShared<dyn ReceiveHandler>> {
    if self.stack.len() <= 1 {
      return None;
    }
    self.stack.pop()
  }

  /// Returns the number of behaviors currently in the stack.
  #[must_use]
  pub fn depth(&self) -> usize {
    self.stack.len()
  }
}
