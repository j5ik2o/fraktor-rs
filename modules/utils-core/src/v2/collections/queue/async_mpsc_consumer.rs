use core::marker::PhantomData;

use super::async_queue::poll_shared;
use crate::{
  collections::queue::QueueError,
  sync::{
    ArcShared,
    async_mutex_like::{AsyncMutexLike, SpinAsyncMutex},
  },
  v2::collections::queue::backend::AsyncQueueBackend,
};

/// Async consumer for queues tagged with
/// [`MpscKey`](crate::v2::collections::queue::type_keys::MpscKey).
pub struct AsyncMpscConsumer<T, B, A = SpinAsyncMutex<B>>
where
  B: AsyncQueueBackend<T>,
  A: AsyncMutexLike<B>, {
  pub(crate) inner: ArcShared<A>,
  _pd:              PhantomData<(T, B)>,
}

impl<T, B, A> AsyncMpscConsumer<T, B, A>
where
  B: AsyncQueueBackend<T>,
  A: AsyncMutexLike<B>,
{
  pub(crate) fn new(inner: ArcShared<A>) -> Self {
    Self { inner, _pd: PhantomData }
  }

  /// Polls the next element from the queue.
  pub async fn poll(&self) -> Result<T, QueueError<T>> {
    poll_shared::<T, B, A>(&self.inner).await
  }

  /// Signals that no more elements will be produced.
  pub async fn close(&self) -> Result<(), QueueError<T>> {
    let mut guard = <A as AsyncMutexLike<B>>::lock(&*self.inner).await.map_err(QueueError::from)?;
    guard.close().await
  }

  /// Returns the number of stored elements.
  #[must_use]
  pub async fn len(&self) -> Result<usize, QueueError<T>> {
    let guard = <A as AsyncMutexLike<B>>::lock(&*self.inner).await.map_err(QueueError::from)?;
    Ok(guard.len())
  }

  /// Returns the queue capacity.
  #[must_use]
  pub async fn capacity(&self) -> Result<usize, QueueError<T>> {
    let guard = <A as AsyncMutexLike<B>>::lock(&*self.inner).await.map_err(QueueError::from)?;
    Ok(guard.capacity())
  }

  /// Indicates whether the queue is empty.
  #[must_use]
  pub async fn is_empty(&self) -> Result<bool, QueueError<T>> {
    let guard = <A as AsyncMutexLike<B>>::lock(&*self.inner).await.map_err(QueueError::from)?;
    Ok(guard.is_empty())
  }

  /// Provides access to the shared backend.
  #[must_use]
  pub fn shared(&self) -> &ArcShared<A> {
    &self.inner
  }
}
