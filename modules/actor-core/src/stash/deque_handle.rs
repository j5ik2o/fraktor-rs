use fraktor_utils_core_rs::collections::queue::QueueError;

mod stash_deque_handle_generic;

pub use stash_deque_handle_generic::{StashDequeHandle, StashDequeHandleGeneric};

#[cfg(test)]
mod tests;

/// Contract describing the double-ended queue operations exposed to stash-aware actors.
pub trait DequeHandle<T>
where
  T: Send + 'static, {
  /// Pushes an element to the front of the deque.
  ///
  /// # Errors
  ///
  /// Returns [`QueueError`] when the underlying queue has been closed or is already full.
  fn push_front(&self, item: T) -> Result<(), QueueError<T>>;

  /// Pushes an element to the back of the deque.
  ///
  /// # Errors
  ///
  /// Returns [`QueueError`] when the underlying queue has been closed or is already full.
  fn push_back(&self, item: T) -> Result<(), QueueError<T>>;

  /// Removes an element from the front of the deque.
  ///
  /// # Errors
  ///
  /// Returns [`QueueError`] when the queue is empty and cannot provide an item anymore.
  fn pop_front(&self) -> Result<T, QueueError<T>>;

  /// Removes an element from the back of the deque.
  ///
  /// # Errors
  ///
  /// Returns [`QueueError`] when the queue is empty and cannot provide an item anymore.
  fn pop_back(&self) -> Result<T, QueueError<T>>;
}
