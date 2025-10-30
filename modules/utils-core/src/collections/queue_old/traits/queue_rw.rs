use crate::collections::queue::QueueError;
use crate::collections::queue_old::traits::queue_base::QueueBase;

/// Trait providing read/write operations for the queue using shared references.
pub trait QueueRw<E>: QueueBase<E> {
  /// Adds an element to the queue (shared reference version).
  ///
  /// # Errors
  ///
  /// Returns a `QueueError` when the element cannot be accepted, such as when the queue is full,
  /// closed, or otherwise unable to enqueue the item.
  fn offer(&self, element: E) -> Result<(), QueueError<E>>;

  /// Removes an element from the queue (shared reference version).
  ///
  /// # Errors
  ///
  /// Returns a `QueueError` when the queue cannot supply an element due to closure, disconnection,
  /// or backend failures.
  fn poll(&self) -> Result<Option<E>, QueueError<E>>;

  /// Performs queue cleanup processing (shared reference version).
  fn clean_up(&self);
}
