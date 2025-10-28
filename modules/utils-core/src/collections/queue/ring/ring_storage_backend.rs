use crate::collections::{
  QueueError, QueueSize,
  queue::{
    ring::ring_backend::RingBackend,
    traits::{QueueBase, QueueHandle, QueueReader, QueueStorage, QueueWriter},
  },
};

/// Backend implementation that directly operates on ring buffer storage handles.
#[derive(Debug)]
pub struct RingStorageBackend<S> {
  storage: S,
}

impl<S> RingStorageBackend<S> {
  /// Creates a new `RingStorageBackend`.
  #[must_use]
  pub const fn new(storage: S) -> Self {
    Self { storage }
  }

  /// Gets a reference to the storage handle.
  #[must_use]
  pub const fn storage(&self) -> &S {
    &self.storage
  }

  /// Consumes this backend and returns the internal storage handle.
  pub fn into_storage(self) -> S {
    self.storage
  }
}

impl<S, E> RingBackend<E> for RingStorageBackend<S>
where
  S: QueueHandle<E>,
{
  fn offer(&self, element: E) -> Result<(), QueueError<E>> {
    self.storage().storage().with_write(|buffer| buffer.offer_mut(element))
  }

  fn poll(&self) -> Result<Option<E>, QueueError<E>> {
    self.storage().storage().with_write(|buffer| buffer.poll_mut())
  }

  fn clean_up(&self) {
    self.storage().storage().with_write(|buffer| buffer.clean_up_mut());
  }

  fn len(&self) -> QueueSize {
    self.storage().storage().with_read(|buffer| buffer.len())
  }

  fn capacity(&self) -> QueueSize {
    self.storage().storage().with_read(|buffer| buffer.capacity())
  }

  fn set_dynamic(&self, dynamic: bool) {
    self.storage().storage().with_write(|buffer| buffer.set_dynamic(dynamic));
  }
}
