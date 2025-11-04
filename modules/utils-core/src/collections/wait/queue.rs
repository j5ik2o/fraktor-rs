use alloc::collections::VecDeque;

use super::{handle_shared::WaitShared, node::WaitNode};
use crate::sync::ArcShared;

#[cfg(all(test, feature = "alloc"))]
mod tests;

/// FIFO queue managing waiter nodes.
pub struct WaitQueue<E> {
  waiters: VecDeque<ArcShared<WaitNode<E>>>,
}

impl<E> WaitQueue<E> {
  /// Creates an empty queue.
  #[must_use]
  pub const fn new() -> Self {
    Self { waiters: VecDeque::new() }
  }

  /// Registers a new waiter and returns a shared future for awaiting completion.
  pub fn register(&mut self) -> WaitShared<E> {
    let node = ArcShared::new(WaitNode::new());
    self.waiters.push_back(node.clone());
    WaitShared::new(node)
  }

  /// Notifies the oldest pending waiter with success.
  pub fn notify_success(&mut self) -> bool {
    while let Some(node) = self.waiters.pop_front() {
      if node.complete_ok() {
        return true;
      }
    }
    false
  }

  /// Completes all waiters with the provided error.
  pub fn notify_error_all(&mut self, error: E)
  where
    E: Clone, {
    self.notify_error_all_with(|| error.clone());
  }

  /// Completes all waiters with errors produced by the supplied closure.
  pub fn notify_error_all_with<F>(&mut self, mut make_error: F)
  where
    F: FnMut() -> E, {
    while let Some(node) = self.waiters.pop_front() {
      node.complete_with_error(make_error());
    }
  }
}

impl<E> Default for WaitQueue<E> {
  fn default() -> Self {
    Self::new()
  }
}
