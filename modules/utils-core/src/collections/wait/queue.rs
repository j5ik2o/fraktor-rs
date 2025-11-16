use super::{WaitError, handle_shared::WaitShared, node::WaitNode};
use crate::{
  collections::queue::{
    QueueError, SyncFifoQueue,
    backend::{OverflowPolicy, VecDequeBackend},
  },
  sync::ArcShared,
};

#[cfg(all(test, feature = "alloc"))]
mod tests;

/// FIFO queue managing waiter nodes.
pub struct WaitQueue<E> {
  waiters: SyncFifoQueue<ArcShared<WaitNode<E>>, VecDequeBackend<ArcShared<WaitNode<E>>>>,
}

impl<E> WaitQueue<E> {
  /// Creates an empty queue.
  #[must_use]
  pub fn new() -> Self {
    let backend = VecDequeBackend::with_capacity(16, OverflowPolicy::Grow);
    Self { waiters: SyncFifoQueue::new_fifo(backend) }
  }

  /// Registers a new waiter and returns a shared future for awaiting completion.
  ///
  /// # Errors
  ///
  /// Returns a `WaitError` if the queue cannot accept the waiter due to allocation failure
  /// or if the queue is closed.
  pub fn register(&mut self) -> Result<WaitShared<E>, WaitError> {
    let node = ArcShared::new(WaitNode::new());
    self.waiters.offer(node.clone()).map_err(|e| match e {
      | QueueError::AllocError(_) => WaitError::AllocationFailure,
      | QueueError::Closed(_) => WaitError::QueueClosed,
      | QueueError::Full(_) => WaitError::AllocationFailure, // Should not happen with Grow policy
      | _ => WaitError::AllocationFailure,                   // Fallback
    })?;
    Ok(WaitShared::new(node))
  }

  /// Notifies the oldest pending waiter with success.
  pub fn notify_success(&mut self) -> bool {
    while let Ok(node) = self.waiters.poll() {
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
    while let Ok(node) = self.waiters.poll() {
      node.complete_with_error(make_error());
    }
  }
}

impl<E> Default for WaitQueue<E> {
  fn default() -> Self {
    Self::new()
  }
}
