use fraktor_utils_rs::core::{
  collections::queue::{
    OfferOutcome, OverflowPolicy, QueueError, SyncFifoQueueShared, SyncQueue, backend::VecDequeBackend,
  },
  sync::{ArcShared, sync_mutex_like::SpinSyncMutex},
};

use super::{StreamBufferConfig, StreamError};

#[cfg(test)]
mod tests;

/// Queue-backed buffer used for backpressure.
pub struct StreamBuffer<T> {
  queue:           SyncFifoQueueShared<T, VecDequeBackend<T>>,
  overflow_policy: OverflowPolicy,
}

impl<T> StreamBuffer<T> {
  /// Creates a new buffer using the provided configuration.
  #[must_use]
  pub fn new(config: StreamBufferConfig) -> Self {
    let backend = VecDequeBackend::with_capacity(config.capacity(), config.overflow_policy());
    let queue = SyncQueue::new(backend);
    let shared = ArcShared::new(SpinSyncMutex::new(queue));
    let queue = SyncFifoQueueShared::new_fifo(shared);
    Self { queue, overflow_policy: config.overflow_policy() }
  }

  /// Attempts to enqueue an element.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when the buffer rejects the element.
  pub fn offer(&self, value: T) -> Result<OfferOutcome, StreamError> {
    self.queue.offer(value).map_err(|error| map_queue_error(&error))
  }

  /// Attempts to dequeue the next element.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when the buffer is empty or closed.
  pub fn poll(&self) -> Result<T, StreamError> {
    self.queue.poll().map_err(|error| map_queue_error(&error))
  }

  /// Returns the number of buffered elements.
  #[must_use]
  pub fn len(&self) -> usize {
    self.queue.len()
  }

  /// Returns `true` when the buffer is empty.
  #[must_use]
  pub fn is_empty(&self) -> bool {
    self.queue.is_empty()
  }

  /// Returns the capacity limit.
  #[must_use]
  pub fn capacity(&self) -> usize {
    self.queue.capacity()
  }

  /// Returns the overflow policy.
  #[must_use]
  pub const fn overflow_policy(&self) -> OverflowPolicy {
    self.overflow_policy
  }
}

const fn map_queue_error<T>(error: &QueueError<T>) -> StreamError {
  match error {
    | QueueError::Full(_) | QueueError::AllocError(_) => StreamError::BufferOverflow,
    | _ => StreamError::Failed,
  }
}
