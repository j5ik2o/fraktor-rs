use super::SyncQueue;
use crate::core::{
  collections::queue::{QueueError, backend::SyncQueueBackend, offer_outcome::OfferOutcome},
  sync::{ArcShared, SpinSyncMutex},
};

/// Shared, mutex-guarded handle around [`SyncQueue`].
///
/// Internally guarded by [`SpinSyncMutex`]; ordering and capability semantics
/// (FIFO / priority / etc.) are entirely determined by the supplied backend.
#[derive(Clone)]
pub struct SyncQueueShared<T, B>
where
  B: SyncQueueBackend<T>, {
  inner: ArcShared<SpinSyncMutex<SyncQueue<T, B>>>,
}

impl<T, B> SyncQueueShared<T, B>
where
  B: SyncQueueBackend<T>,
{
  /// Creates a new shared queue from the provided shared backend.
  #[must_use]
  pub const fn new(shared_queue: ArcShared<SpinSyncMutex<SyncQueue<T, B>>>) -> Self {
    Self { inner: shared_queue }
  }

  /// Creates a shared queue by materializing the built-in spin lock locally.
  #[must_use]
  pub fn new_with_builtin_lock(queue: SyncQueue<T, B>) -> Self {
    Self { inner: ArcShared::new(SpinSyncMutex::new(queue)) }
  }

  /// Enqueues an item according to the backend's overflow policy.
  ///
  /// # Errors
  ///
  /// Returns a `QueueError` when the backend rejects the element because the queue is closed,
  /// full, or disconnected.
  pub fn offer(&self, item: T) -> Result<OfferOutcome, QueueError<T>> {
    let mut queue = self.inner.lock();
    queue.offer(item)
  }

  /// Dequeues the next available item.
  ///
  /// # Errors
  ///
  /// Returns a `QueueError` when the backend cannot supply an element due to closure,
  /// disconnection, or backend-specific failures.
  pub fn poll(&self) -> Result<T, QueueError<T>> {
    let mut queue = self.inner.lock();
    queue.poll()
  }

  /// Requests the backend to transition into the closed state.
  ///
  /// # Errors
  ///
  /// Returns a `QueueError` when the backend refuses to close.
  pub fn close(&self) -> Result<(), QueueError<T>> {
    let mut queue = self.inner.lock();
    queue.close()
  }

  /// Returns the current number of stored elements.
  #[must_use]
  pub fn len(&self) -> usize {
    let queue = self.inner.lock();
    queue.len()
  }

  /// Returns the storage capacity.
  #[must_use]
  pub fn capacity(&self) -> usize {
    let queue = self.inner.lock();
    queue.capacity()
  }

  /// Indicates whether the queue is empty.
  #[must_use]
  pub fn is_empty(&self) -> bool {
    self.len() == 0
  }

  /// Indicates whether the queue is full.
  #[must_use]
  pub fn is_full(&self) -> bool {
    self.len() == self.capacity()
  }

  /// Provides access to the underlying shared backend.
  #[must_use]
  pub const fn shared(&self) -> &ArcShared<SpinSyncMutex<SyncQueue<T, B>>> {
    &self.inner
  }
}
