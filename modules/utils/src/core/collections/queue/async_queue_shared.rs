use core::marker::PhantomData;

use super::{
  AsyncQueue, async_mpsc_consumer_shared::AsyncMpscConsumerShared, async_mpsc_producer_shared::AsyncMpscProducerShared,
  async_spsc_consumer_shared::AsyncSpscConsumerShared, async_spsc_producer_shared::AsyncSpscProducerShared,
};
use crate::core::{
  collections::{
    PriorityMessage,
    queue::{
      QueueError,
      backend::{AsyncPriorityBackend, AsyncQueueBackend, OfferOutcome},
      capabilities::{MultiProducer, SingleConsumer, SingleProducer, SupportsPeek},
      type_keys::{FifoKey, MpscKey, PriorityKey, SpscKey, TypeKey},
    },
  },
  sync::{
    ArcShared,
    async_mutex_like::{AsyncMutexLike, SpinAsyncMutex},
  },
};

#[cfg(test)]
mod tests;

pub(crate) async fn offer_shared<T, K, B, A>(shared: &ArcShared<A>, item: T) -> Result<OfferOutcome, QueueError<T>>
where
  K: TypeKey,
  B: AsyncQueueBackend<T>,
  A: AsyncMutexLike<AsyncQueue<T, K, B>>, {
  let mut value = Some(item);

  loop {
    let mut guard = <A as AsyncMutexLike<AsyncQueue<T, K, B>>>::lock(&**shared).await.map_err(QueueError::from)?;

    if guard.is_closed() {
      let Some(item) = value.take() else {
        return Err(QueueError::Disconnected);
      };
      return Err(QueueError::Closed(item));
    }

    if guard.is_full() {
      if let Some(waiter) = guard.prepare_producer_wait().map_err(|_| QueueError::Disconnected)? {
        drop(guard);

        match waiter.await {
          | Ok(()) => continue,
          | Err(err) => return Err(err),
        }
      } else {
        drop(guard);
        let Some(item) = value.take() else {
          return Err(QueueError::Disconnected);
        };
        return Err(QueueError::Full(item));
      }
    } else {
      let Some(item) = value.take() else {
        return Err(QueueError::Disconnected);
      };
      let result = guard.offer(item).await;
      drop(guard);
      return result;
    }
  }
}

pub(crate) async fn poll_shared<T, K, B, A>(shared: &ArcShared<A>) -> Result<T, QueueError<T>>
where
  K: TypeKey,
  B: AsyncQueueBackend<T>,
  A: AsyncMutexLike<AsyncQueue<T, K, B>>, {
  loop {
    let mut guard = <A as AsyncMutexLike<AsyncQueue<T, K, B>>>::lock(&**shared).await.map_err(QueueError::from)?;

    if guard.is_empty() {
      if guard.is_closed() {
        drop(guard);
        return Err(QueueError::Disconnected);
      }

      if let Some(waiter) = guard.prepare_consumer_wait().map_err(|_| QueueError::Disconnected)? {
        drop(guard);

        match waiter.await {
          | Ok(()) => continue,
          | Err(err) => return Err(err),
        }
      } else {
        drop(guard);
        return Err(QueueError::Empty);
      }
    } else {
      let result = guard.poll().await;
      drop(guard);
      return result;
    }
  }
}

/// Async queue API wrapping a shared queue guarded by an async-capable mutex.
pub struct AsyncQueueShared<T, K, B, A = SpinAsyncMutex<AsyncQueue<T, K, B>>>
where
  K: TypeKey,
  B: AsyncQueueBackend<T>,
  A: AsyncMutexLike<AsyncQueue<T, K, B>>, {
  inner: ArcShared<A>,
  _pd:   PhantomData<(T, K, B)>,
}

impl<T, K, B, A> Clone for AsyncQueueShared<T, K, B, A>
where
  K: TypeKey,
  B: AsyncQueueBackend<T>,
  A: AsyncMutexLike<AsyncQueue<T, K, B>>,
{
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone(), _pd: PhantomData }
  }
}

impl<T, K, B, A> AsyncQueueShared<T, K, B, A>
where
  K: TypeKey,
  B: AsyncQueueBackend<T>,
  A: AsyncMutexLike<AsyncQueue<T, K, B>>,
{
  /// Creates a new async queue from the provided shared queue.
  #[must_use]
  pub const fn new(shared_queue: ArcShared<A>) -> Self {
    Self { inner: shared_queue, _pd: PhantomData }
  }

  /// Adds an element to the queue according to the backend's policy.
  ///
  /// # Errors
  ///
  /// Returns a `QueueError` when the backend rejects the element, such as when the queue is closed,
  /// full, or disconnected.
  pub async fn offer(&self, item: T) -> Result<OfferOutcome, QueueError<T>> {
    offer_shared::<T, K, B, A>(&self.inner, item).await
  }

  /// Removes and returns the next available item.
  ///
  /// # Errors
  ///
  /// Returns a `QueueError` when the backend cannot supply an item due to closure, disconnection,
  /// or backend-specific failures.
  pub async fn poll(&self) -> Result<T, QueueError<T>> {
    poll_shared::<T, K, B, A>(&self.inner).await
  }

  /// Requests the backend to transition into the closed state.
  ///
  /// # Errors
  ///
  /// Returns a `QueueError` when the backend refuses to close.
  pub async fn close(&self) -> Result<(), QueueError<T>> {
    let mut guard = <A as AsyncMutexLike<AsyncQueue<T, K, B>>>::lock(&*self.inner).await.map_err(QueueError::from)?;
    guard.close().await
  }

  /// Returns the current number of stored elements.
  ///
  /// # Errors
  ///
  /// Returns a `QueueError` when the backend cannot report its length.
  pub async fn len(&self) -> Result<usize, QueueError<T>> {
    let guard = <A as AsyncMutexLike<AsyncQueue<T, K, B>>>::lock(&*self.inner).await.map_err(QueueError::from)?;
    Ok(guard.len())
  }

  /// Returns the storage capacity.
  ///
  /// # Errors
  ///
  /// Returns a `QueueError` when the backend cannot expose its capacity.
  pub async fn capacity(&self) -> Result<usize, QueueError<T>> {
    let guard = <A as AsyncMutexLike<AsyncQueue<T, K, B>>>::lock(&*self.inner).await.map_err(QueueError::from)?;
    Ok(guard.capacity())
  }

  /// Indicates whether the queue is empty.
  ///
  /// # Errors
  ///
  /// Returns a `QueueError` when the backend cannot determine emptiness due to closure or
  /// disconnection.
  pub async fn is_empty(&self) -> Result<bool, QueueError<T>> {
    let guard = <A as AsyncMutexLike<AsyncQueue<T, K, B>>>::lock(&*self.inner).await.map_err(QueueError::from)?;
    Ok(guard.is_empty())
  }

  /// Indicates whether the queue is full.
  ///
  /// # Errors
  ///
  /// Returns a `QueueError` when the backend cannot determine fullness.
  pub async fn is_full(&self) -> Result<bool, QueueError<T>> {
    let guard = <A as AsyncMutexLike<AsyncQueue<T, K, B>>>::lock(&*self.inner).await.map_err(QueueError::from)?;
    Ok(guard.is_full())
  }

  /// Provides access to the underlying shared queue.
  #[must_use]
  pub const fn shared(&self) -> &ArcShared<A> {
    &self.inner
  }
}

impl<T, B, A> AsyncQueueShared<T, PriorityKey, B, A>
where
  T: Clone + PriorityMessage,
  B: AsyncPriorityBackend<T>,
  A: AsyncMutexLike<AsyncQueue<T, PriorityKey, B>>,
  PriorityKey: SupportsPeek,
{
  /// Retrieves the smallest element without removing it.
  ///
  /// # Errors
  ///
  /// Returns a `QueueError` when the backend cannot access the next element due to closure or
  /// disconnection.
  pub async fn peek_min(&self) -> Result<Option<T>, QueueError<T>> {
    let guard =
      <A as AsyncMutexLike<AsyncQueue<T, PriorityKey, B>>>::lock(&*self.inner).await.map_err(QueueError::from)?;
    guard.peek_min()
  }
}

impl<T, B, A> AsyncQueueShared<T, MpscKey, B, A>
where
  B: AsyncQueueBackend<T>,
  A: AsyncMutexLike<AsyncQueue<T, MpscKey, B>>,
  MpscKey: MultiProducer + SingleConsumer,
{
  /// Creates an async queue tailored for MPSC usage.
  #[must_use]
  pub const fn new_mpsc(shared_queue: ArcShared<A>) -> Self {
    Self::new(shared_queue)
  }

  /// Returns a cloneable producer for MPSC usage.
  #[must_use]
  pub fn producer_clone(&self) -> AsyncMpscProducerShared<T, B, A> {
    AsyncMpscProducerShared::new(self.inner.clone())
  }

  /// Consumes the queue and returns the producer/consumer pair.
  #[must_use]
  pub fn into_mpsc_pair(self) -> (AsyncMpscProducerShared<T, B, A>, AsyncMpscConsumerShared<T, B, A>) {
    let consumer = AsyncMpscConsumerShared::new(self.inner.clone());
    let producer = AsyncMpscProducerShared::new(self.inner);
    (producer, consumer)
  }
}

impl<T, B, A> AsyncQueueShared<T, SpscKey, B, A>
where
  B: AsyncQueueBackend<T>,
  A: AsyncMutexLike<AsyncQueue<T, SpscKey, B>>,
  SpscKey: SingleProducer + SingleConsumer,
{
  /// Creates an async queue tailored for SPSC usage.
  #[must_use]
  pub const fn new_spsc(shared_queue: ArcShared<A>) -> Self {
    Self::new(shared_queue)
  }

  /// Consumes the queue and returns the SPSC producer/consumer pair.
  #[must_use]
  pub fn into_spsc_pair(self) -> (AsyncSpscProducerShared<T, B, A>, AsyncSpscConsumerShared<T, B, A>) {
    let consumer = AsyncSpscConsumerShared::new(self.inner.clone());
    let producer = AsyncSpscProducerShared::new(self.inner);
    (producer, consumer)
  }
}

impl<T, B, A> AsyncQueueShared<T, FifoKey, B, A>
where
  B: AsyncQueueBackend<T>,
  A: AsyncMutexLike<AsyncQueue<T, FifoKey, B>>,
  FifoKey: SingleProducer + SingleConsumer,
{
  /// Creates an async queue tailored for FIFO usage.
  #[must_use]
  pub const fn new_fifo(shared_queue: ArcShared<A>) -> Self {
    Self::new(shared_queue)
  }
}

/// Type alias for an async MPSC queue.
pub type AsyncMpscQueueShared<T, B, A = SpinAsyncMutex<AsyncQueue<T, MpscKey, B>>> = AsyncQueueShared<T, MpscKey, B, A>;
/// Type alias for an async SPSC queue.
pub type AsyncSpscQueueShared<T, B, A = SpinAsyncMutex<AsyncQueue<T, SpscKey, B>>> = AsyncQueueShared<T, SpscKey, B, A>;
/// Type alias for an async FIFO queue.
pub type AsyncFifoQueueShared<T, B, A = SpinAsyncMutex<AsyncQueue<T, FifoKey, B>>> = AsyncQueueShared<T, FifoKey, B, A>;
/// Type alias for an async priority queue.
pub type AsyncPriorityQueueShared<T, B, A = SpinAsyncMutex<AsyncQueue<T, PriorityKey, B>>> =
  AsyncQueueShared<T, PriorityKey, B, A>;
