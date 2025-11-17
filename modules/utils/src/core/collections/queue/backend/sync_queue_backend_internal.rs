use crate::core::collections::queue::{OfferOutcome, OverflowPolicy, QueueError};

/// Internal Backend trait responsible for queue operations.
pub(crate) trait SyncQueueBackendInternal<T> {
  /// Adds an element to the queue according to the configured overflow policy.
  ///
  /// # Errors
  ///
  /// Returns a `QueueError` when the backend rejects the element because the queue is closed,
  /// full, or disconnected.
  fn offer(&mut self, item: T) -> Result<OfferOutcome, QueueError<T>>;

  /// Removes and returns the next element from the queue.
  ///
  /// # Errors
  ///
  /// Returns a `QueueError` when the backend cannot supply an element due to closure,
  /// disconnection, or backend-specific failures.
  fn poll(&mut self) -> Result<T, QueueError<T>>;

  /// Returns the number of elements currently stored.
  fn len(&self) -> usize;

  /// Returns the maximum number of elements that can be stored without growing.
  fn capacity(&self) -> usize;

  /// Indicates whether the queue is empty.
  #[allow(dead_code)]
  fn is_empty(&self) -> bool {
    self.len() == 0
  }

  /// Indicates whether the queue is full.
  #[allow(dead_code)]
  fn is_full(&self) -> bool {
    self.len() == self.capacity()
  }

  /// Returns the overflow policy currently configured for the backend.
  fn overflow_policy(&self) -> OverflowPolicy;

  /// Indicates whether the backend has been closed.
  fn is_closed(&self) -> bool {
    false
  }

  /// Closes the backend, preventing further offers while allowing in-flight polls to complete.
  fn close(&mut self) {}
}
