use crate::collections::queue_old::{queue_error::QueueError, traits::queue_base::QueueBase};

/// Trait providing write operations to the queue for mutable references.
pub trait QueueWriter<E>: QueueBase<E> {
  /// Adds an element to the queue (mutable reference version).
  ///
  /// # Errors
  ///
  /// Returns a `QueueError` when the queue rejects the element because it is full, closed, or
  /// encounters backend-specific failures.
  fn offer_mut(&mut self, element: E) -> Result<(), QueueError<E>>;
}
