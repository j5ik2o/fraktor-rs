use crate::collections::{QueueSize, queue::QueueError};

/// Backend abstraction trait for ring buffer-based queues.
pub trait RingBackend<E> {
  /// Adds an element to the queue.
  ///
  /// # Errors
  ///
  /// Returns a `QueueError` when the element cannot be accepted because the queue is full, closed,
  /// or disconnected.
  fn offer(&self, element: E) -> Result<(), QueueError<E>>;

  /// Removes an element from the queue.
  ///
  /// # Errors
  ///
  /// Returns a `QueueError` when the queue cannot provide an element due to closure,
  /// disconnection, or backend failures.
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
