use crate::collections::queue::{queue_error::QueueError, traits::queue_base::QueueBase};

/// Trait providing write operations to the queue for mutable references.
pub trait QueueWriter<E>: QueueBase<E> {
  /// Adds an element to the queue (mutable reference version).
  fn offer_mut(&mut self, element: E) -> Result<(), QueueError<E>>;
}
