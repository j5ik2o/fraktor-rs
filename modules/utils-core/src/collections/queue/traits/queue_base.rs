use crate::collections::queue::queue_size::QueueSize;

/// Common trait defining basic queue operations.
pub trait QueueBase<E> {
  /// Returns the current size of the queue.
  fn len(&self) -> QueueSize;

  /// Returns the queue capacity.
  fn capacity(&self) -> QueueSize;

  /// Checks if the queue is empty.
  #[must_use]
  fn is_empty(&self) -> bool {
    self.len() == QueueSize::Limited(0)
  }
}
