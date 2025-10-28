use super::traits::{MpscBackend, MpscHandle};
use crate::collections::{QueueBase, QueueError, QueueReader, QueueRw, QueueSize, QueueWriter};
#[cfg(test)]
mod tests;

/// Queue facade that operates on an [`MpscBackend`]
///
/// This queue implements the multi-producer, single-consumer (MPSC) pattern,
/// allowing multiple threads to add elements and a single thread to retrieve them.
#[derive(Debug)]
pub struct MpscQueue<S, T>
where
  S: MpscHandle<T>, {
  storage: S,
  _marker: core::marker::PhantomData<T>,
}

impl<S, T> MpscQueue<S, T>
where
  S: MpscHandle<T>,
{
  /// Creates a new [`MpscQueue`] using the specified storage.
  ///
  /// # Arguments
  ///
  /// * `storage` - The backend storage for the queue
  ///
  /// # Returns
  ///
  /// A new [`MpscQueue`] instance
  pub const fn new(storage: S) -> Self {
    Self { storage, _marker: core::marker::PhantomData }
  }

  /// Gets a reference to the backend storage.
  ///
  /// # Returns
  ///
  /// An immutable reference to the storage
  pub const fn storage(&self) -> &S {
    &self.storage
  }

  /// Extracts the backend storage from the queue.
  ///
  /// This method consumes the [`MpscQueue`] and transfers ownership to the storage.
  ///
  /// # Returns
  ///
  /// The backend storage
  pub fn into_storage(self) -> S {
    self.storage
  }

  /// Sets the capacity of the queue.
  ///
  /// # Arguments
  ///
  /// * `capacity` - The new capacity. `None` means unlimited.
  ///
  /// # Returns
  ///
  /// `true` if the capacity was successfully set, `false` otherwise
  pub fn set_capacity(&self, capacity: Option<usize>) -> bool {
    self.storage.backend().set_capacity(capacity)
  }

  /// Adds an element to the queue.
  ///
  /// # Arguments
  ///
  /// * `element` - The element to add to the queue
  ///
  /// # Returns
  ///
  /// * `Ok(())` - Element was successfully added
  ///
  /// # Errors
  ///
  /// * `QueueError::Full(element)` - Queue is full
  /// * `QueueError::Closed(element)` - Queue is closed
  pub fn offer(&self, element: T) -> Result<(), QueueError<T>> {
    self.storage.backend().try_send(element)
  }

  /// Retrieves an element from the queue.
  ///
  /// # Returns
  ///
  /// * `Ok(Some(element))` - Element was successfully retrieved
  /// * `Ok(None)` - Queue is empty
  ///
  /// # Errors
  ///
  /// * `QueueError::Disconnected` - Queue is closed
  pub fn poll(&self) -> Result<Option<T>, QueueError<T>> {
    self.storage.backend().try_recv()
  }

  /// Cleans up and closes the queue.
  ///
  /// After calling this method, subsequent `offer` operations will fail,
  /// and `poll` operations will return an error after retrieving remaining elements.
  pub fn clean_up(&self) {
    self.storage.backend().close();
  }

  /// Gets the capacity of the queue.
  ///
  /// # Returns
  ///
  /// The queue capacity. [`QueueSize::Limitless`] if unlimited
  pub fn capacity(&self) -> QueueSize {
    self.storage.backend().capacity()
  }

  /// Checks whether the queue is closed.
  ///
  /// # Returns
  ///
  /// `true` if the queue is closed, `false` otherwise
  pub fn is_closed(&self) -> bool {
    self.storage.backend().is_closed()
  }

  /// Gets a reference to the backend (internal use).
  ///
  /// # Returns
  ///
  /// A reference to the backend
  fn backend(&self) -> &S::Backend {
    self.storage.backend()
  }
}

impl<S, T> Clone for MpscQueue<S, T>
where
  S: MpscHandle<T>,
{
  /// Creates a clone of the queue.
  ///
  /// The backend storage is shared, so the cloned queue references
  /// the same queue instance.
  ///
  /// # Returns
  ///
  /// A new [`MpscQueue`] instance sharing the same backend storage
  fn clone(&self) -> Self {
    Self { storage: self.storage.clone(), _marker: core::marker::PhantomData }
  }
}

impl<S, T> QueueBase<T> for MpscQueue<S, T>
where
  S: MpscHandle<T>,
{
  /// Gets the number of elements in the queue.
  ///
  /// # Returns
  ///
  /// The number of elements in the queue. [`QueueSize::Limitless`] if unlimited
  fn len(&self) -> QueueSize {
    self.backend().len()
  }

  /// Gets the capacity of the queue.
  ///
  /// # Returns
  ///
  /// The queue capacity. [`QueueSize::Limitless`] if unlimited
  fn capacity(&self) -> QueueSize {
    self.capacity()
  }
}

impl<S, T> QueueWriter<T> for MpscQueue<S, T>
where
  S: MpscHandle<T>,
{
  /// Adds an element to the queue using a mutable reference.
  ///
  /// Returns an error if the queue is full or closed.
  ///
  /// # Arguments
  ///
  /// * `element` - The element to add to the queue
  ///
  /// # Returns
  ///
  /// * `Ok(())` - Element was successfully added
  /// * `Err(QueueError::Full(element))` - Queue is full
  /// * `Err(QueueError::Closed(element))` - Queue is closed
  fn offer_mut(&mut self, element: T) -> Result<(), QueueError<T>> {
    self.backend().try_send(element)
  }
}

impl<S, T> QueueReader<T> for MpscQueue<S, T>
where
  S: MpscHandle<T>,
{
  /// Retrieves an element from the queue using a mutable reference.
  ///
  /// Returns `None` if the queue is empty. Returns an error if the queue is closed.
  ///
  /// # Returns
  ///
  /// * `Ok(Some(element))` - Element was successfully retrieved
  /// * `Ok(None)` - Queue is empty
  /// * `Err(QueueError::Disconnected)` - Queue is closed
  fn poll_mut(&mut self) -> Result<Option<T>, QueueError<T>> {
    self.backend().try_recv()
  }

  /// Cleans up and closes the queue using a mutable reference.
  ///
  /// After calling this method, subsequent `offer_mut` operations will fail,
  /// and `poll_mut` operations will return an error after retrieving remaining elements.
  fn clean_up_mut(&mut self) {
    self.backend().close();
  }
}

impl<S, T> QueueRw<T> for MpscQueue<S, T>
where
  S: MpscHandle<T>,
{
  /// Adds an element to the queue using a shared reference.
  ///
  /// Returns an error if the queue is full or closed.
  ///
  /// # Arguments
  ///
  /// * `element` - The element to add to the queue
  ///
  /// # Returns
  ///
  /// * `Ok(())` - Element was successfully added
  /// * `Err(QueueError::Full(element))` - Queue is full
  /// * `Err(QueueError::Closed(element))` - Queue is closed
  fn offer(&self, element: T) -> Result<(), QueueError<T>> {
    self.offer(element)
  }

  /// Retrieves an element from the queue using a shared reference.
  ///
  /// Returns `None` if the queue is empty. Returns an error if the queue is closed.
  ///
  /// # Returns
  ///
  /// * `Ok(Some(element))` - Element was successfully retrieved
  /// * `Ok(None)` - Queue is empty
  /// * `Err(QueueError::Disconnected)` - Queue is closed
  fn poll(&self) -> Result<Option<T>, QueueError<T>> {
    self.poll()
  }

  /// Cleans up and closes the queue using a shared reference.
  ///
  /// After calling this method, subsequent `offer` operations will fail,
  /// and `poll` operations will return an error after retrieving remaining elements.
  fn clean_up(&self) {
    self.clean_up();
  }
}
