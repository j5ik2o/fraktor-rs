use crate::collections::queue::{queue_error::QueueError, traits::queue_base::QueueBase};

/// Trait providing read operations from the queue for mutable references.
pub trait QueueReader<E>: QueueBase<E> {
  /// Removes an element from the queue (mutable reference version).
  ///
  /// # Errors
  ///
  /// Returns a `QueueError` when the underlying queue cannot provide an element due to being
  /// closed, disconnected, or encountering backend-specific failures.
  fn poll_mut(&mut self) -> Result<Option<E>, QueueError<E>>;

  /// Performs queue cleanup processing (mutable reference version).
  fn clean_up_mut(&mut self);
}
