use core::marker::PhantomData;

use super::{
  sync_mpsc_consumer::SyncMpscConsumer, sync_mpsc_producer::SyncMpscProducer, sync_spsc_consumer::SyncSpscConsumer,
  sync_spsc_producer::SyncSpscProducer,
};
use crate::{
  PriorityMessage,
  collections::queue::{
    QueueError,
    backend::{OfferOutcome, SyncQueueBackend, sync_priority_backend::SyncPriorityBackend},
    capabilities::{MultiProducer, SingleConsumer, SingleProducer, SupportsPeek},
    type_keys::{FifoKey, MpscKey, PriorityKey, SpscKey, TypeKey},
  },
  sync::{
    ArcShared, Shared, SharedAccess,
    sync_mutex_like::{SpinSyncMutex, SyncMutexLike},
  },
};

/// Queue API parameterised by element type, type key, backend, and shared guard.
#[derive(Clone)]
pub struct SyncQueue<T, K, B, M = SpinSyncMutex<B>>
where
  K: TypeKey,
  B: SyncQueueBackend<T>,
  M: SyncMutexLike<B>, {
  inner: ArcShared<M>,
  _pd:   PhantomData<(T, K, B)>,
}

impl<T, K, B, M> SyncQueue<T, K, B, M>
where
  K: TypeKey,
  B: SyncQueueBackend<T>,
  M: SyncMutexLike<B>,
  ArcShared<M>: SharedAccess<B>,
{
  /// Creates a new queue from the provided shared backend.
  #[must_use]
  pub const fn new(shared_backend: ArcShared<M>) -> Self {
    Self { inner: shared_backend, _pd: PhantomData }
  }

  /// Enqueues an item according to the backend's overflow policy.
  ///
  /// # Errors
  ///
  /// Returns a `QueueError` when the backend rejects the element because the queue is closed,
  /// full, or disconnected.
  pub fn offer(&self, item: T) -> Result<OfferOutcome, QueueError<T>> {
    self.inner.with_mut(|backend: &mut B| backend.offer(item)).map_err(QueueError::from)?
  }

  /// Dequeues the next available item.
  ///
  /// # Errors
  ///
  /// Returns a `QueueError` when the backend cannot supply an element due to closure,
  /// disconnection, or backend-specific failures.
  pub fn poll(&self) -> Result<T, QueueError<T>> {
    self.inner.with_mut(|backend: &mut B| backend.poll()).map_err(QueueError::from)?
  }

  /// Requests the backend to transition into the closed state.
  ///
  /// # Errors
  ///
  /// Returns a `QueueError` when the backend refuses to close.
  pub fn close(&self) -> Result<(), QueueError<T>> {
    self
      .inner
      .with_mut(|backend: &mut B| {
        backend.close();
        Ok(())
      })
      .map_err(QueueError::from)?
  }

  /// Returns the current number of stored elements.
  #[must_use]
  pub fn len(&self) -> usize {
    self.inner.with_ref(|mutex: &M| {
      let guard = mutex.lock();
      guard.len()
    })
  }

  /// Returns the storage capacity.
  #[must_use]
  pub fn capacity(&self) -> usize {
    self.inner.with_ref(|mutex: &M| {
      let guard = mutex.lock();
      guard.capacity()
    })
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
  pub const fn shared(&self) -> &ArcShared<M> {
    &self.inner
  }
}

impl<T, B, M> SyncQueue<T, PriorityKey, B, M>
where
  T: Clone + PriorityMessage,
  B: SyncPriorityBackend<T>,
  M: SyncMutexLike<B>,
  ArcShared<M>: SharedAccess<B>,
  PriorityKey: SupportsPeek,
{
  /// Retrieves the smallest element without removing it.
  ///
  /// # Errors
  ///
  /// Returns a `QueueError` when the backend cannot access the next element due to closure,
  /// disconnection, or backend-specific failures.
  pub fn peek_min(&self) -> Result<Option<T>, QueueError<T>> {
    self.inner.with_mut(|backend: &mut B| Ok(backend.peek_min().cloned())).map_err(QueueError::from)?
  }
}

impl<T, B, M> SyncQueue<T, MpscKey, B, M>
where
  B: SyncQueueBackend<T>,
  M: SyncMutexLike<B>,
  ArcShared<M>: SharedAccess<B>,
  MpscKey: MultiProducer + SingleConsumer,
{
  /// Creates a queue tailored for MPSC usage.
  #[must_use]
  pub const fn new_mpsc(shared_backend: ArcShared<M>) -> Self {
    SyncQueue::new(shared_backend)
  }

  /// Returns a cloneable producer for MPSC usage.
  #[must_use]
  pub fn producer_clone(&self) -> SyncMpscProducer<T, B, M> {
    SyncMpscProducer::new(self.inner.clone())
  }

  /// Consumes the queue and returns the producer/consumer pair.
  #[must_use]
  pub fn into_mpsc_pair(self) -> (SyncMpscProducer<T, B, M>, SyncMpscConsumer<T, B, M>) {
    let consumer = SyncMpscConsumer::new(self.inner.clone());
    let producer = SyncMpscProducer::new(self.inner);
    (producer, consumer)
  }
}

impl<T, B, M> SyncQueue<T, SpscKey, B, M>
where
  B: SyncQueueBackend<T>,
  M: SyncMutexLike<B>,
  ArcShared<M>: SharedAccess<B>,
  SpscKey: SingleProducer + SingleConsumer,
{
  /// Creates a queue tailored for SPSC usage.
  #[must_use]
  pub const fn new_spsc(shared_backend: ArcShared<M>) -> Self {
    SyncQueue::new(shared_backend)
  }

  /// Consumes the queue and returns the SPSC producer/consumer pair.
  #[must_use]
  pub fn into_spsc_pair(self) -> (SyncSpscProducer<T, B, M>, SyncSpscConsumer<T, B, M>) {
    let consumer = SyncSpscConsumer::new(self.inner.clone());
    let producer = SyncSpscProducer::new(self.inner);
    (producer, consumer)
  }
}

impl<T, B, M> SyncQueue<T, FifoKey, B, M>
where
  B: SyncQueueBackend<T>,
  M: SyncMutexLike<B>,
  ArcShared<M>: SharedAccess<B>,
  FifoKey: SingleProducer + SingleConsumer,
{
  /// Creates a queue tailored for FIFO usage.
  #[must_use]
  pub const fn new_fifo(shared_backend: ArcShared<M>) -> Self {
    SyncQueue::new(shared_backend)
  }
}

/// Type alias for an MPSC queue.
pub type MpscQueue<T, B, M = SpinSyncMutex<B>> = SyncQueue<T, MpscKey, B, M>;
/// Type alias for an SPSC queue.
pub type SpscQueue<T, B, M = SpinSyncMutex<B>> = SyncQueue<T, SpscKey, B, M>;
/// Type alias for a FIFO queue.
pub type FifoQueue<T, B, M = SpinSyncMutex<B>> = SyncQueue<T, FifoKey, B, M>;
/// Type alias for a priority queue.
pub type PriorityQueue<T, B, M = SpinSyncMutex<B>> = SyncQueue<T, PriorityKey, B, M>;
