#[cfg(test)]
mod tests;

use core::marker::PhantomData;

use super::{AsyncQueue, async_queue_shared::poll_shared};
use crate::{
  collections::queue::{QueueError, backend::AsyncQueueBackend, type_keys::MpscKey},
  sync::{
    ArcShared,
    async_mutex_like::{AsyncMutexLike, SpinAsyncMutex},
  },
};

/// Async consumer for queues tagged with
/// [`MpscKey`](crate::collections::queue::type_keys::MpscKey).
pub struct AsyncMpscConsumerShared<T, B, A = SpinAsyncMutex<AsyncQueue<T, MpscKey, B>>>
where
  B: AsyncQueueBackend<T>,
  A: AsyncMutexLike<AsyncQueue<T, MpscKey, B>>, {
  pub(crate) inner: ArcShared<A>,
  _pd:              PhantomData<(T, B)>,
}

impl<T, B, A> AsyncMpscConsumerShared<T, B, A>
where
  B: AsyncQueueBackend<T>,
  A: AsyncMutexLike<AsyncQueue<T, MpscKey, B>>,
{
  pub(crate) const fn new(inner: ArcShared<A>) -> Self {
    Self { inner, _pd: PhantomData }
  }

  /// Polls the next element from the queue.
  ///
  /// # Errors
  ///
  /// Returns a `QueueError` when the backend cannot produce the next element because it is closed,
  /// disconnected, or encounters backend-specific failures.
  pub async fn poll(&self) -> Result<T, QueueError<T>> {
    poll_shared::<T, MpscKey, B, A>(&self.inner).await
  }

  /// Signals that no more elements will be produced.
  ///
  /// # Errors
  ///
  /// Returns a `QueueError` when the backend refuses to transition into the closed state.
  pub async fn close(&self) -> Result<(), QueueError<T>> {
    let mut guard =
      <A as AsyncMutexLike<AsyncQueue<T, MpscKey, B>>>::lock(&*self.inner).await.map_err(QueueError::from)?;
    guard.close().await
  }

  /// Returns the number of stored elements.
  ///
  /// # Errors
  ///
  /// Returns a `QueueError` when the backend cannot report its length due to closure or
  /// disconnection.
  pub async fn len(&self) -> Result<usize, QueueError<T>> {
    let guard = <A as AsyncMutexLike<AsyncQueue<T, MpscKey, B>>>::lock(&*self.inner).await.map_err(QueueError::from)?;
    Ok(guard.len())
  }

  /// Returns the queue capacity.
  ///
  /// # Errors
  ///
  /// Returns a `QueueError` when the backend cannot expose its capacity information.
  pub async fn capacity(&self) -> Result<usize, QueueError<T>> {
    let guard = <A as AsyncMutexLike<AsyncQueue<T, MpscKey, B>>>::lock(&*self.inner).await.map_err(QueueError::from)?;
    Ok(guard.capacity())
  }

  /// Indicates whether the queue is empty.
  ///
  /// # Errors
  ///
  /// Returns a `QueueError` when the backend cannot determine emptiness due to closure or
  /// disconnection.
  pub async fn is_empty(&self) -> Result<bool, QueueError<T>> {
    let guard = <A as AsyncMutexLike<AsyncQueue<T, MpscKey, B>>>::lock(&*self.inner).await.map_err(QueueError::from)?;
    Ok(guard.is_empty())
  }

  /// Provides access to the shared queue.
  #[must_use]
  pub const fn shared(&self) -> &ArcShared<A> {
    &self.inner
  }
}
