#[cfg(test)]
mod tests;

use core::marker::PhantomData;

use crate::{
  QueueError, QueueSize,
  collections::queue_old::{
    ring::{ring_backend::RingBackend, ring_handle::RingHandle},
    traits::{QueueBase, QueueReader, QueueRw, QueueWriter},
  },
};

/// Queue facade that delegates all operations to a [`RingBackend`].
#[derive(Debug)]
pub struct RingQueue<H, E>
where
  H: RingHandle<E>, {
  backend: H,
  _marker: PhantomData<E>,
}

impl<H, E> RingQueue<H, E>
where
  H: RingHandle<E>,
{
  /// Creates a new `RingQueue` from the specified backend handle.
  ///
  /// # Arguments
  ///
  /// * `backend` - Backend handle for the ring queue
  ///
  /// # Returns
  ///
  /// New `RingQueue` instance
  #[must_use]
  pub const fn new(backend: H) -> Self {
    Self { backend, _marker: PhantomData }
  }

  /// Returns a reference to the backend handle.
  ///
  /// # Returns
  ///
  /// Immutable reference to the backend handle
  pub const fn backend(&self) -> &H {
    &self.backend
  }

  /// Consumes this queue and returns the internal backend handle.
  ///
  /// # Returns
  ///
  /// Internal backend handle
  pub fn into_backend(self) -> H {
    self.backend
  }

  /// Sets the dynamic mode of the queue.
  ///
  /// When dynamic mode is enabled, the queue automatically expands when elements are added beyond
  /// capacity.
  ///
  /// # Arguments
  ///
  /// * `dynamic` - If `true`, enables dynamic mode
  pub fn set_dynamic(&self, dynamic: bool) {
    self.backend.backend().set_dynamic(dynamic);
  }

  /// Builder method that sets dynamic mode and returns this queue.
  ///
  /// When dynamic mode is enabled, the queue automatically expands when elements are added beyond
  /// capacity.
  ///
  /// # Arguments
  ///
  /// * `dynamic` - If `true`, enables dynamic mode
  ///
  /// # Returns
  ///
  /// This queue with dynamic mode set
  #[must_use]
  pub fn with_dynamic(self, dynamic: bool) -> Self {
    self.set_dynamic(dynamic);
    self
  }

  /// Adds an element to the queue.
  ///
  /// # Arguments
  ///
  /// * `element` - Element to add to the queue
  ///
  /// # Returns
  ///
  /// * `Ok(())` - If element was successfully added
  /// * `Err(QueueError)` - If queue is full and element cannot be added
  ///
  /// # Errors
  ///
  /// Returns `QueueError::Full` if the queue is full and dynamic mode is disabled.
  pub fn offer(&self, element: E) -> Result<(), QueueError<E>> {
    self.backend.backend().offer(element)
  }

  /// Removes an element from the queue.
  ///
  /// # Returns
  ///
  /// * `Ok(Some(E))` - Element removed from the queue
  /// * `Ok(None)` - If queue is empty
  /// * `Err(QueueError)` - If an error occurred
  ///
  /// # Errors
  ///
  /// This method currently does not return any errors, but the signature allows for future error
  /// handling.
  pub fn poll(&self) -> Result<Option<E>, QueueError<E>> {
    self.backend.backend().poll()
  }

  /// Performs queue cleanup.
  ///
  /// Performs maintenance such as internal buffer memory optimization.
  pub fn clean_up(&self) {
    self.backend.backend().clean_up();
  }
}

impl<H, E> Clone for RingQueue<H, E>
where
  H: RingHandle<E>,
{
  /// Creates a clone of the `RingQueue`.
  ///
  /// Clones the backend handle and returns a new `RingQueue` instance.
  ///
  /// # Returns
  ///
  /// Clone of this queue
  fn clone(&self) -> Self {
    Self { backend: self.backend.clone(), _marker: PhantomData }
  }
}

impl<H, E> QueueBase<E> for RingQueue<H, E>
where
  H: RingHandle<E>,
{
  /// Returns the number of elements in the queue.
  ///
  /// # Returns
  ///
  /// Current number of elements in the queue
  fn len(&self) -> QueueSize {
    self.backend.backend().len()
  }

  /// Returns the capacity of the queue.
  ///
  /// # Returns
  ///
  /// Maximum number of elements the queue can hold
  fn capacity(&self) -> QueueSize {
    self.backend.backend().capacity()
  }
}

impl<H, E> QueueWriter<E> for RingQueue<H, E>
where
  H: RingHandle<E>,
{
  /// Adds an element to the queue using a mutable reference.
  ///
  /// # Arguments
  ///
  /// * `element` - Element to add to the queue
  ///
  /// # Returns
  ///
  /// * `Ok(())` - If element was successfully added
  /// * `Err(QueueError)` - If queue is full and element cannot be added
  ///
  /// # Errors
  ///
  /// Returns `QueueError::Full` if the queue is full and dynamic mode is disabled.
  fn offer_mut(&mut self, element: E) -> Result<(), QueueError<E>> {
    self.offer(element)
  }
}

impl<H, E> QueueReader<E> for RingQueue<H, E>
where
  H: RingHandle<E>,
{
  /// Removes an element from the queue using a mutable reference.
  ///
  /// # Returns
  ///
  /// * `Ok(Some(E))` - Element removed from the queue
  /// * `Ok(None)` - If queue is empty
  /// * `Err(QueueError)` - If an error occurred
  ///
  /// # Errors
  ///
  /// This method currently does not return any errors, but the signature allows for future error
  /// handling.
  fn poll_mut(&mut self) -> Result<Option<E>, QueueError<E>> {
    self.poll()
  }

  /// Performs queue cleanup using a mutable reference.
  ///
  /// Performs maintenance such as internal buffer memory optimization.
  fn clean_up_mut(&mut self) {
    self.clean_up();
  }
}

impl<H, E> QueueRw<E> for RingQueue<H, E>
where
  H: RingHandle<E>,
{
  /// Adds an element to the queue using a shared reference.
  ///
  /// # Arguments
  ///
  /// * `element` - Element to add to the queue
  ///
  /// # Returns
  ///
  /// * `Ok(())` - If element was successfully added
  /// * `Err(QueueError)` - If queue is full and element cannot be added
  ///
  /// # Errors
  ///
  /// Returns `QueueError::Full` if the queue is full and dynamic mode is disabled.
  fn offer(&self, element: E) -> Result<(), QueueError<E>> {
    self.offer(element)
  }

  /// Removes an element from the queue using a shared reference.
  ///
  /// # Returns
  ///
  /// * `Ok(Some(E))` - Element removed from the queue
  /// * `Ok(None)` - If queue is empty
  /// * `Err(QueueError)` - If an error occurred
  ///
  /// # Errors
  ///
  /// This method currently does not return any errors, but the signature allows for future error
  /// handling.
  fn poll(&self) -> Result<Option<E>, QueueError<E>> {
    self.poll()
  }

  /// Performs queue cleanup using a shared reference.
  ///
  /// Performs maintenance such as internal buffer memory optimization.
  fn clean_up(&self) {
    self.clean_up();
  }
}
