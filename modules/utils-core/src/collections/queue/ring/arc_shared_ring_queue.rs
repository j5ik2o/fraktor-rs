use crate::{
  QueueError, QueueSize,
  collections::queue::{
    ring::{
      DEFAULT_CAPACITY, RingBuffer, RingQueue, ring_handle::RingHandle, ring_storage_backend::RingStorageBackend,
    },
    traits::{QueueBase, QueueHandle, QueueReader, QueueRw, QueueWriter},
  },
  sync::{ArcShared, sync_mutex_like::SpinSyncMutex},
};

#[cfg(test)]
mod tests;

type SharedRingStorage<E> = ArcShared<RingStorageBackend<ArcShared<SpinSyncMutex<RingBuffer<E>>>>>;

impl<E> RingHandle<E> for SharedRingStorage<E> {
  type Backend = RingStorageBackend<ArcShared<SpinSyncMutex<RingBuffer<E>>>>;

  fn backend(&self) -> &Self::Backend {
    self
  }
}

impl<E> QueueHandle<E> for ArcShared<SpinSyncMutex<RingBuffer<E>>> {
  type Storage = SpinSyncMutex<RingBuffer<E>>;

  fn storage(&self) -> &Self::Storage {
    self
  }
}

/// RingQueue wrapper backed by [`ArcShared`] + [`SpinSyncMutex`].
#[derive(Clone)]
pub struct ArcSharedRingQueue<E> {
  inner: RingQueue<SharedRingStorage<E>, E>,
}

impl<E> ArcSharedRingQueue<E> {
  /// Creates a new queue with the given capacity.
  #[must_use]
  pub fn new(capacity: usize) -> Self {
    let storage = ArcShared::new(SpinSyncMutex::new(RingBuffer::new(capacity)));
    let backend = ArcShared::new(RingStorageBackend::new(storage));
    Self { inner: RingQueue::new(backend) }
  }

  /// Builder-style helper to toggle dynamic expansion.
  #[must_use]
  pub fn with_dynamic(mut self, dynamic: bool) -> Self {
    self.inner = self.inner.with_dynamic(dynamic);
    self
  }

  /// Updates the dynamic expansion flag.
  pub fn set_dynamic(&self, dynamic: bool) {
    self.inner.set_dynamic(dynamic);
  }
}

impl<E> Default for ArcSharedRingQueue<E> {
  fn default() -> Self {
    Self::new(DEFAULT_CAPACITY)
  }
}

impl<E> QueueBase<E> for ArcSharedRingQueue<E> {
  fn len(&self) -> QueueSize {
    self.inner.len()
  }

  fn capacity(&self) -> QueueSize {
    self.inner.capacity()
  }
}

impl<E> QueueWriter<E> for ArcSharedRingQueue<E> {
  fn offer_mut(&mut self, element: E) -> Result<(), QueueError<E>> {
    self.inner.offer_mut(element)
  }
}

impl<E> QueueReader<E> for ArcSharedRingQueue<E> {
  fn poll_mut(&mut self) -> Result<Option<E>, QueueError<E>> {
    self.inner.poll_mut()
  }

  fn clean_up_mut(&mut self) {
    self.inner.clean_up_mut();
  }
}

impl<E> QueueRw<E> for ArcSharedRingQueue<E> {
  fn offer(&self, element: E) -> Result<(), QueueError<E>> {
    self.inner.offer(element)
  }

  fn poll(&self) -> Result<Option<E>, QueueError<E>> {
    self.inner.poll()
  }

  fn clean_up(&self) {
    self.inner.clean_up();
  }
}
