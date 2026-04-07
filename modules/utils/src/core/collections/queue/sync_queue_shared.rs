use core::marker::PhantomData;

use super::SyncQueue;
use crate::core::{
  collections::queue::{
    QueueError,
    backend::SyncQueueBackend,
    capabilities::{SingleConsumer, SingleProducer},
    offer_outcome::OfferOutcome,
    type_keys::{FifoKey, TypeKey},
  },
  sync::{ArcShared, SpinSyncMutex},
};

/// Queue API parameterised by element type, type key, and backend.
///
/// Internally guarded by [`SpinSyncMutex`]; the mutex backend is fixed
/// after the sync abstraction collapse.
#[derive(Clone)]
pub struct SyncQueueShared<T, K, B>
where
  K: TypeKey,
  B: SyncQueueBackend<T>, {
  inner: ArcShared<SpinSyncMutex<SyncQueue<T, K, B>>>,
  _pd:   PhantomData<(T, K, B)>,
}

impl<T, K, B> SyncQueueShared<T, K, B>
where
  K: TypeKey,
  B: SyncQueueBackend<T>,
{
  /// Creates a new queue from the provided shared backend.
  #[must_use]
  pub const fn new(shared_queue: ArcShared<SpinSyncMutex<SyncQueue<T, K, B>>>) -> Self {
    Self { inner: shared_queue, _pd: PhantomData }
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
  pub const fn shared(&self) -> &ArcShared<SpinSyncMutex<SyncQueue<T, K, B>>> {
    &self.inner
  }
}

impl<T, B> SyncQueueShared<T, FifoKey, B>
where
  B: SyncQueueBackend<T>,
  FifoKey: SingleProducer + SingleConsumer,
{
  /// Creates a queue tailored for FIFO usage.
  #[must_use]
  pub const fn new_fifo(shared_queue: ArcShared<SpinSyncMutex<SyncQueue<T, FifoKey, B>>>) -> Self {
    SyncQueueShared::new(shared_queue)
  }
}

/// Type alias for a FIFO queue.
pub type SyncFifoQueueShared<T, B> = SyncQueueShared<T, FifoKey, B>;
