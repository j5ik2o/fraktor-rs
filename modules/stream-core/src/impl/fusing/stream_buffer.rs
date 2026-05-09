use fraktor_utils_core_rs::{
  collections::queue::{OfferOutcome, QueueError, SyncQueue, SyncQueueShared, backend::VecDequeBackend},
  sync::{ArcShared, SpinSyncMutex},
};

use super::{StreamError, stream_buffer_config::StreamBufferConfig};

#[cfg(test)]
mod tests;

/// Queue-backed buffer used for backpressure.
pub(crate) struct StreamBuffer<T> {
  queue: SyncQueueShared<T, VecDequeBackend<T>>,
}

impl<T> StreamBuffer<T> {
  /// Creates a new buffer using the provided configuration.
  #[must_use]
  pub(crate) fn new(config: StreamBufferConfig) -> Self {
    let backend = VecDequeBackend::with_capacity(config.capacity(), config.overflow_policy());
    let queue = SyncQueue::new(backend);
    let shared = ArcShared::new(SpinSyncMutex::new(queue));
    let queue = SyncQueueShared::new(shared);
    Self { queue }
  }

  /// Attempts to enqueue an element.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when the buffer rejects the element.
  pub(crate) fn offer(&self, value: T) -> Result<OfferOutcome, StreamError> {
    self.queue.offer(value).map_err(|error| map_queue_error(&error))
  }

  /// Attempts to dequeue the next element.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when the buffer is empty or closed.
  pub(crate) fn poll(&self) -> Result<T, StreamError> {
    self.queue.poll().map_err(|error| map_queue_error(&error))
  }

  /// Returns `true` when the buffer is empty.
  #[must_use]
  pub(crate) fn is_empty(&self) -> bool {
    self.queue.is_empty()
  }
}

const fn map_queue_error<T>(error: &QueueError<T>) -> StreamError {
  match error {
    | QueueError::Full(_) | QueueError::AllocError(_) => StreamError::BufferOverflow,
    | _ => StreamError::Failed,
  }
}
