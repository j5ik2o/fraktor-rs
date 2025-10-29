use super::traits::MpscBackend;
use crate::collections::{QueueError, QueueSize, queue_old::ring::RingBufferStorage};

/// Backend implementation that drives a shared multi-producer/single-consumer
/// queue using ring buffer storage.
///
/// This backend utilizes in-memory [`RingBufferStorage`] to achieve efficient
/// message delivery from multiple producers to a single consumer.
#[derive(Debug)]
pub struct RingBufferBackend<S> {
  storage: S,
}

impl<S> RingBufferBackend<S> {
  /// Creates a new `RingBufferBackend` using the specified storage.
  ///
  /// # Arguments
  ///
  /// * `storage` - The ring buffer storage used by the backend
  ///
  /// # Returns
  ///
  /// A new `RingBufferBackend` instance
  pub const fn new(storage: S) -> Self {
    Self { storage }
  }

  /// Returns an immutable reference to the internal storage.
  ///
  /// # Returns
  ///
  /// A reference to the storage
  pub const fn storage(&self) -> &S {
    &self.storage
  }

  /// Takes ownership of and returns the internal storage.
  ///
  /// This method consumes the backend and returns the internal storage.
  ///
  /// # Returns
  ///
  /// Ownership of the internal storage
  pub fn into_storage(self) -> S {
    self.storage
  }
}

impl<S, T> MpscBackend<T> for RingBufferBackend<S>
where
  S: RingBufferStorage<T>,
{
  /// Attempts to send an element to the queue.
  ///
  /// This method tries to add an element to the queue without blocking.
  /// Returns an error if the queue is full or closed.
  ///
  /// # Arguments
  ///
  /// * `element` - The element to add to the queue
  ///
  /// # Returns
  ///
  /// * `Ok(())` - If the element was successfully added to the queue
  /// * `Err(QueueError<T>)` - If the queue is full or closed
  fn try_send(&self, element: T) -> Result<(), QueueError<T>> {
    self.storage.with_write(|buffer| buffer.offer(element))
  }

  /// Attempts to receive an element from the queue.
  ///
  /// This method tries to retrieve an element from the queue without blocking.
  /// Returns `None` if the queue is empty.
  ///
  /// # Returns
  ///
  /// * `Ok(Some(T))` - If an element was successfully retrieved
  /// * `Ok(None)` - If the queue is empty
  /// * `Err(QueueError<T>)` - If an error occurred
  fn try_recv(&self) -> Result<Option<T>, QueueError<T>> {
    self.storage.with_write(|buffer| buffer.poll())
  }

  /// Closes the queue and cleans up resources.
  ///
  /// After calling this method, no new sends to the queue will be possible.
  /// Elements already in the queue can still be received.
  fn close(&self) {
    self.storage.with_write(|buffer| buffer.clean_up());
  }

  /// Returns the current number of elements in the queue.
  ///
  /// # Returns
  ///
  /// A [`QueueSize`] representing the number of elements in the queue
  fn len(&self) -> QueueSize {
    self.storage.with_read(|buffer| buffer.len())
  }

  /// Returns the capacity of the queue.
  ///
  /// # Returns
  ///
  /// A [`QueueSize`] representing the capacity of the queue (`QueueSize::Unbounded` if unlimited)
  fn capacity(&self) -> QueueSize {
    self.storage.with_read(|buffer| buffer.capacity())
  }

  /// Checks whether the queue is closed.
  ///
  /// # Returns
  ///
  /// * `true` - If the queue is closed
  /// * `false` - If the queue is open
  fn is_closed(&self) -> bool {
    self.storage.with_read(|buffer| buffer.is_closed())
  }

  /// Sets the capacity of the queue.
  ///
  /// This method changes the maximum capacity of the queue.
  ///
  /// # Arguments
  ///
  /// * `capacity` - The new capacity (`None` for unlimited)
  ///
  /// # Returns
  ///
  /// * `true` - If the capacity was successfully set
  fn set_capacity(&self, capacity: Option<usize>) -> bool {
    self.storage.with_write(|buffer| buffer.set_capacity(capacity));
    true
  }
}
