use crate::collections::{QueueError, QueueSize};

/// Backend abstraction trait for ring buffer-based queues.
pub trait RingBackend<E> {
  /// Adds an element to the queue.
  fn offer(&self, element: E) -> Result<(), QueueError<E>>;

  /// Removes an element from the queue.
  fn poll(&self) -> Result<Option<E>, QueueError<E>>;

  /// Cleans up the queue's internal state.
  fn clean_up(&self);

  /// Returns the number of elements currently stored in the queue.
  fn len(&self) -> QueueSize;

  /// Returns the queue capacity (maximum storable count).
  fn capacity(&self) -> QueueSize;

  /// Enables or disables the queue's dynamic resizing feature.
  fn set_dynamic(&self, dynamic: bool);

  /// Checks if the queue is empty.
  #[must_use]
  fn is_empty(&self) -> bool {
    self.len() == QueueSize::Limited(0)
  }
}
