use crate::collections::{QueueError, QueueSize};

/// Transport-oriented trait abstracting MPSC queue backends.
pub trait MpscBackend<T> {
  /// Attempts to send an element to the queue (non-blocking).
  fn try_send(&self, element: T) -> Result<(), QueueError<T>>;

  /// Attempts to receive an element from the queue (non-blocking).
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
