use core::marker::PhantomData;

use crate::collections::{
  PriorityMessage,
  queue::{
    QueueError,
    backend::{AsyncPriorityBackend, AsyncQueueBackend, OfferOutcome},
    capabilities::{MultiProducer, SingleConsumer, SingleProducer, SupportsPeek},
    type_keys::{FifoKey, MpscKey, PriorityKey, SpscKey, TypeKey},
  },
  wait::{WaitError, WaitShared},
};

/// Async queue API parameterised by element type, type key, and backend.
#[derive(Clone)]
pub struct AsyncQueue<T, K, B>
where
  K: TypeKey,
  B: AsyncQueueBackend<T>, {
  backend: B,
  _pd:     PhantomData<(T, K)>,
}

impl<T, K, B> AsyncQueue<T, K, B>
where
  K: TypeKey,
  B: AsyncQueueBackend<T>,
{
  /// Creates a new async queue from the provided backend.
  #[must_use]
  pub const fn new(backend: B) -> Self {
    Self { backend, _pd: PhantomData }
  }

  /// Enqueues an item according to the backend's overflow policy.
  ///
  /// # Errors
  ///
  /// Returns a `QueueError` when the backend rejects the element because the queue is closed,
  /// full, or disconnected.
  pub async fn offer(&mut self, item: T) -> Result<OfferOutcome, QueueError<T>> {
    self.backend.offer(item).await
  }

  /// Dequeues the next available item.
  ///
  /// # Errors
  ///
  /// Returns a `QueueError` when the backend cannot supply an element due to closure,
  /// disconnection, or backend-specific failures.
  pub async fn poll(&mut self) -> Result<T, QueueError<T>> {
    self.backend.poll().await
  }

  /// Requests the backend to transition into the closed state.
  ///
  /// # Errors
  ///
  /// Returns a `QueueError` when the backend refuses to close.
  pub async fn close(&mut self) -> Result<(), QueueError<T>> {
    self.backend.close().await
  }

  /// Returns the current number of stored elements.
  #[must_use]
  pub fn len(&self) -> usize {
    self.backend.len()
  }

  /// Returns the storage capacity.
  #[must_use]
  pub fn capacity(&self) -> usize {
    self.backend.capacity()
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

  /// Provides access to the underlying backend.
  #[must_use]
  pub const fn backend(&self) -> &B {
    &self.backend
  }

  /// Indicates whether the backend has transitioned into the closed state.
  #[must_use]
  pub fn is_closed(&self) -> bool {
    self.backend.is_closed()
  }

  /// Optionally registers a producer waiter when the queue is full.
  ///
  /// # Errors
  ///
  /// Returns a `WaitError` when the waiter cannot be registered.
  pub fn prepare_producer_wait(&mut self) -> Result<Option<WaitShared<QueueError<T>>>, WaitError> {
    self.backend.prepare_producer_wait()
  }

  /// Optionally registers a consumer waiter when the queue is empty.
  ///
  /// # Errors
  ///
  /// Returns a `WaitError` when the waiter cannot be registered.
  pub fn prepare_consumer_wait(&mut self) -> Result<Option<WaitShared<QueueError<T>>>, WaitError> {
    self.backend.prepare_consumer_wait()
  }
}

impl<T, B> AsyncQueue<T, PriorityKey, B>
where
  T: Clone + PriorityMessage,
  B: AsyncPriorityBackend<T>,
  PriorityKey: SupportsPeek,
{
  /// Retrieves the smallest element without removing it.
  ///
  /// # Errors
  ///
  /// Returns a `QueueError` when the backend cannot access the next element due to closure,
  /// disconnection, or backend-specific failures.
  pub fn peek_min(&self) -> Result<Option<T>, QueueError<T>> {
    Ok(self.backend.peek_min().cloned())
  }
}

impl<T, B> AsyncQueue<T, MpscKey, B>
where
  B: AsyncQueueBackend<T>,
  MpscKey: MultiProducer + SingleConsumer,
{
  /// Creates a queue tailored for MPSC usage.
  #[must_use]
  pub const fn new_mpsc(backend: B) -> Self {
    AsyncQueue::new(backend)
  }
}

impl<T, B> AsyncQueue<T, SpscKey, B>
where
  B: AsyncQueueBackend<T>,
  SpscKey: SingleProducer + SingleConsumer,
{
  /// Creates a queue tailored for SPSC usage.
  #[must_use]
  pub const fn new_spsc(backend: B) -> Self {
    AsyncQueue::new(backend)
  }
}

impl<T, B> AsyncQueue<T, FifoKey, B>
where
  B: AsyncQueueBackend<T>,
  FifoKey: SingleProducer + SingleConsumer,
{
  /// Creates a queue tailored for FIFO usage.
  #[must_use]
  pub const fn new_fifo(backend: B) -> Self {
    AsyncQueue::new(backend)
  }
}

/// Type alias for an async MPSC queue.
pub type AsyncMpscQueue<T, B> = AsyncQueue<T, MpscKey, B>;
/// Type alias for an async SPSC queue.
pub type AsyncSpscQueue<T, B> = AsyncQueue<T, SpscKey, B>;
/// Type alias for an async FIFO queue.
pub type AsyncFifoQueue<T, B> = AsyncQueue<T, FifoKey, B>;
/// Type alias for an async priority queue.
pub type AsyncPriorityQueue<T, B> = AsyncQueue<T, PriorityKey, B>;
