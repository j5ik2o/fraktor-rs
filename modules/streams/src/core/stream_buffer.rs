//! Stream buffer implementation backed by shared queue primitives.

#[cfg(test)]
mod tests;

use fraktor_utils_rs::core::collections::queue::{OverflowPolicy, QueueError, SyncFifoQueue, backend::VecDequeBackend};

use crate::core::stream_error::StreamError;

/// Buffer for stream elements.
pub struct StreamBuffer<T> {
  queue: SyncFifoQueue<T, VecDequeBackend<T>>,
}

impl<T> StreamBuffer<T> {
  /// Creates a new buffer with capacity and overflow policy.
  #[must_use]
  pub fn new(capacity: usize, policy: OverflowPolicy) -> Self {
    let backend = VecDequeBackend::with_capacity(capacity, policy);
    Self { queue: SyncFifoQueue::new(backend) }
  }

  /// Attempts to enqueue a value into the buffer.
  ///
  /// # Errors
  ///
  /// Returns `StreamError::BufferFull` when the buffer is full, or another buffer error otherwise.
  pub fn offer(&mut self, value: T) -> Result<(), StreamError> {
    self.queue.offer(value).map(|_| ()).map_err(|error| map_queue_error(&error))
  }

  /// Attempts to dequeue a value from the buffer.
  ///
  /// # Errors
  ///
  /// Returns `StreamError::BufferEmpty` when the buffer is empty, or another buffer error
  /// otherwise.
  pub fn poll(&mut self) -> Result<T, StreamError> {
    self.queue.poll().map_err(|error| map_queue_error(&error))
  }

  /// Returns the number of buffered elements.
  #[must_use]
  pub fn len(&self) -> usize {
    self.queue.len()
  }

  /// Returns true when the buffer is empty.
  #[must_use]
  pub fn is_empty(&self) -> bool {
    self.queue.is_empty()
  }

  /// Returns the buffer capacity.
  #[must_use]
  pub fn capacity(&self) -> usize {
    self.queue.capacity()
  }
}

const fn map_queue_error<T>(error: &QueueError<T>) -> StreamError {
  match error {
    | QueueError::Full(_) => StreamError::BufferFull,
    | QueueError::Closed(_) => StreamError::BufferClosed,
    | QueueError::Empty => StreamError::BufferEmpty,
    | QueueError::Disconnected => StreamError::BufferDisconnected,
    | QueueError::AllocError(_) => StreamError::BufferAllocation,
    | QueueError::WouldBlock => StreamError::BufferWouldBlock,
    | QueueError::OfferError(_) => StreamError::BufferFull,
    | QueueError::TimedOut(_) => StreamError::BufferFull,
  }
}
