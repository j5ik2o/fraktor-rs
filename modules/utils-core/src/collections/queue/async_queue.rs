use core::marker::PhantomData;

use super::{
  async_mpsc_consumer::AsyncMpscConsumer, async_mpsc_producer::AsyncMpscProducer,
  async_spsc_consumer::AsyncSpscConsumer, async_spsc_producer::AsyncSpscProducer,
};
use crate::{
  collections::queue_old::QueueError,
  sync::{
    async_mutex_like::{AsyncMutexLike, SpinAsyncMutex},
    ArcShared,
  },
};
use crate::collections::queue::{
  backend::{AsyncPriorityBackend, AsyncQueueBackend, OfferOutcome},
  capabilities::{MultiProducer, SingleConsumer, SingleProducer, SupportsPeek},
  type_keys::{FifoKey, MpscKey, PriorityKey, SpscKey, TypeKey},
};

#[cfg(test)]
mod tests;

pub(crate) async fn offer_shared<T, B, A>(shared: &ArcShared<A>, item: T) -> Result<OfferOutcome, QueueError<T>>
where
  B: AsyncQueueBackend<T>,
  A: AsyncMutexLike<B>, {
  let mut value = Some(item);

  loop {
    let mut guard = <A as AsyncMutexLike<B>>::lock(&**shared).await.map_err(QueueError::from)?;

    if guard.is_closed() {
      let Some(item) = value.take() else {
        return Err(QueueError::Disconnected);
      };
      return Err(QueueError::Closed(item));
    }

    if guard.is_full() {
      if let Some(waiter) = guard.prepare_producer_wait() {
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

pub(crate) async fn poll_shared<T, B, A>(shared: &ArcShared<A>) -> Result<T, QueueError<T>>
where
  B: AsyncQueueBackend<T>,
  A: AsyncMutexLike<B>, {
  loop {
    let mut guard = <A as AsyncMutexLike<B>>::lock(&**shared).await.map_err(QueueError::from)?;

    if guard.is_empty() {
      if guard.is_closed() {
        drop(guard);
        return Err(QueueError::Disconnected);
      }

      if let Some(waiter) = guard.prepare_consumer_wait() {
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

/// Async queue API wrapping a shared backend guarded by an async-capable mutex.
pub struct AsyncQueue<T, K, B, A = SpinAsyncMutex<B>>
where
  K: TypeKey,
  B: AsyncQueueBackend<T>,
  A: AsyncMutexLike<B>, {
  inner: ArcShared<A>,
  _pd:   PhantomData<(T, K, B)>,
}

impl<T, K, B, A> Clone for AsyncQueue<T, K, B, A>
where
  K: TypeKey,
  B: AsyncQueueBackend<T>,
  A: AsyncMutexLike<B>,
{
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone(), _pd: PhantomData }
  }
}

impl<T, K, B, A> AsyncQueue<T, K, B, A>
where
  K: TypeKey,
  B: AsyncQueueBackend<T>,
  A: AsyncMutexLike<B>,
{
  /// Creates a new async queue from the provided shared backend.
  #[must_use]
  pub const fn new(shared_backend: ArcShared<A>) -> Self {
    Self { inner: shared_backend, _pd: PhantomData }
  }

  /// Adds an element to the queue according to the backend's policy.
  ///
  /// # Errors
  ///
  /// Returns a `QueueError` when the backend rejects the element, such as when the queue is closed,
  /// full, or disconnected.
  pub async fn offer(&self, item: T) -> Result<OfferOutcome, QueueError<T>> {
    offer_shared::<T, B, A>(&self.inner, item).await
  }

  /// Removes and returns the next available item.
  ///
  /// # Errors
  ///
  /// Returns a `QueueError` when the backend cannot supply an item due to closure, disconnection,
  /// or backend-specific failures.
  pub async fn poll(&self) -> Result<T, QueueError<T>> {
    poll_shared::<T, B, A>(&self.inner).await
  }

  /// Requests the backend to transition into the closed state.
  ///
  /// # Errors
  ///
  /// Returns a `QueueError` when the backend refuses to close.
  pub async fn close(&self) -> Result<(), QueueError<T>> {
    let mut guard = <A as AsyncMutexLike<B>>::lock(&*self.inner).await.map_err(QueueError::from)?;
    guard.close().await
  }

  /// Returns the current number of stored elements.
  ///
  /// # Errors
  ///
  /// Returns a `QueueError` when the backend cannot report its length.
  pub async fn len(&self) -> Result<usize, QueueError<T>> {
    let guard = <A as AsyncMutexLike<B>>::lock(&*self.inner).await.map_err(QueueError::from)?;
    Ok(guard.len())
  }

  /// Returns the storage capacity.
  ///
  /// # Errors
  ///
  /// Returns a `QueueError` when the backend cannot expose its capacity.
  pub async fn capacity(&self) -> Result<usize, QueueError<T>> {
    let guard = <A as AsyncMutexLike<B>>::lock(&*self.inner).await.map_err(QueueError::from)?;
    Ok(guard.capacity())
  }

  /// Indicates whether the queue is empty.
  ///
  /// # Errors
  ///
  /// Returns a `QueueError` when the backend cannot determine emptiness due to closure or
  /// disconnection.
  pub async fn is_empty(&self) -> Result<bool, QueueError<T>> {
    let guard = <A as AsyncMutexLike<B>>::lock(&*self.inner).await.map_err(QueueError::from)?;
    Ok(guard.is_empty())
  }

  /// Indicates whether the queue is full.
  ///
  /// # Errors
  ///
  /// Returns a `QueueError` when the backend cannot determine fullness.
  pub async fn is_full(&self) -> Result<bool, QueueError<T>> {
    let guard = <A as AsyncMutexLike<B>>::lock(&*self.inner).await.map_err(QueueError::from)?;
    Ok(guard.is_full())
  }

  /// Provides access to the underlying shared backend.
  #[must_use]
  pub const fn shared(&self) -> &ArcShared<A> {
    &self.inner
  }
}

impl<T, B, A> AsyncQueue<T, PriorityKey, B, A>
where
  T: Clone + Ord,
  B: AsyncPriorityBackend<T>,
  A: AsyncMutexLike<B>,
  PriorityKey: SupportsPeek,
{
  /// Retrieves the smallest element without removing it.
  ///
  /// # Errors
  ///
  /// Returns a `QueueError` when the backend cannot access the next element due to closure or
  /// disconnection.
  pub async fn peek_min(&self) -> Result<Option<T>, QueueError<T>> {
    let guard = <A as AsyncMutexLike<B>>::lock(&*self.inner).await.map_err(QueueError::from)?;
    Ok(guard.peek_min().cloned())
  }
}

impl<T, B, A> AsyncQueue<T, MpscKey, B, A>
where
  B: AsyncQueueBackend<T>,
  A: AsyncMutexLike<B>,
  MpscKey: MultiProducer + SingleConsumer,
{
  /// Creates an async queue tailored for MPSC usage.
  #[must_use]
  pub const fn new_mpsc(shared_backend: ArcShared<A>) -> Self {
    Self::new(shared_backend)
  }

  /// Returns a cloneable producer for MPSC usage.
  #[must_use]
  pub fn producer_clone(&self) -> AsyncMpscProducer<T, B, A> {
    AsyncMpscProducer::new(self.inner.clone())
  }

  /// Consumes the queue and returns the producer/consumer pair.
  #[must_use]
  pub fn into_mpsc_pair(self) -> (AsyncMpscProducer<T, B, A>, AsyncMpscConsumer<T, B, A>) {
    let consumer = AsyncMpscConsumer::new(self.inner.clone());
    let producer = AsyncMpscProducer::new(self.inner);
    (producer, consumer)
  }
}

impl<T, B, A> AsyncQueue<T, SpscKey, B, A>
where
  B: AsyncQueueBackend<T>,
  A: AsyncMutexLike<B>,
  SpscKey: SingleProducer + SingleConsumer,
{
  /// Creates an async queue tailored for SPSC usage.
  #[must_use]
  pub const fn new_spsc(shared_backend: ArcShared<A>) -> Self {
    Self::new(shared_backend)
  }

  /// Consumes the queue and returns the SPSC producer/consumer pair.
  #[must_use]
  pub fn into_spsc_pair(self) -> (AsyncSpscProducer<T, B, A>, AsyncSpscConsumer<T, B, A>) {
    let consumer = AsyncSpscConsumer::new(self.inner.clone());
    let producer = AsyncSpscProducer::new(self.inner);
    (producer, consumer)
  }
}

impl<T, B, A> AsyncQueue<T, FifoKey, B, A>
where
  B: AsyncQueueBackend<T>,
  A: AsyncMutexLike<B>,
  FifoKey: SingleProducer + SingleConsumer,
{
  /// Creates an async queue tailored for FIFO usage.
  #[must_use]
  pub const fn new_fifo(shared_backend: ArcShared<A>) -> Self {
    Self::new(shared_backend)
  }
}

/// Type alias for an async MPSC queue.
pub type AsyncMpscQueue<T, B, A = SpinAsyncMutex<B>> = AsyncQueue<T, MpscKey, B, A>;
/// Type alias for an async SPSC queue.
pub type AsyncSpscQueue<T, B, A = SpinAsyncMutex<B>> = AsyncQueue<T, SpscKey, B, A>;
/// Type alias for an async FIFO queue.
pub type AsyncFifoQueue<T, B, A = SpinAsyncMutex<B>> = AsyncQueue<T, FifoKey, B, A>;
/// Type alias for an async priority queue.
pub type AsyncPriorityQueue<T, B, A = SpinAsyncMutex<B>> = AsyncQueue<T, PriorityKey, B, A>;
