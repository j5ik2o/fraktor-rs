use crate::collections::{QueueSize, queue::QueueError};

/// Transport-oriented trait abstracting MPSC queue backends.
pub trait MpscBackend<T> {
  /// Attempts to send an element to the queue (non-blocking).
  ///
  /// # Errors
  ///
  /// Returns a `QueueError` when the element cannot be queued because the backend is closed,
  /// full, disconnected, or otherwise unable to accept items.
  fn try_send(&self, element: T) -> Result<(), QueueError<T>>;

  /// Attempts to receive an element from the queue (non-blocking).
  ///
  /// # Errors
  ///
  /// Returns a `QueueError` when the backend cannot deliver an element due to closure,
  /// disconnection, or backend-specific failures.
  fn try_recv(&self) -> Result<Option<T>, QueueError<T>>;

  /// Closes the queue.
  fn close(&self);

  /// Gets the number of elements currently in the queue.
  fn len(&self) -> QueueSize;

  /// Gets the capacity of the queue.
  fn capacity(&self) -> QueueSize;

  /// Checks if the queue is closed.
  fn is_closed(&self) -> bool;

  /// Sets the capacity of the queue. Defaults to `false`.
  fn set_capacity(&self, capacity: Option<usize>) -> bool {
    let _ = capacity;
    false
  }

  /// Checks if the queue is empty.
  fn is_empty(&self) -> bool {
    self.len() == QueueSize::Limited(0)
  }
}
