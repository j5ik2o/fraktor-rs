use core::marker::PhantomData;

use super::async_queue::poll_shared;
use crate::{
  collections::{queue::backend::AsyncQueueBackend, queue_old::QueueError},
  sync::{
    ArcShared,
    async_mutex_like::{AsyncMutexLike, SpinAsyncMutex},
  },
};

/// Async consumer for queues tagged with
/// [`SpscKey`](crate::collections::queue::type_keys::SpscKey).
pub struct AsyncSpscConsumer<T, B, A = SpinAsyncMutex<B>>
where
  B: AsyncQueueBackend<T>,
  A: AsyncMutexLike<B>, {
  pub(crate) inner: ArcShared<A>,
  _pd:              PhantomData<(T, B)>,
}

impl<T, B, A> AsyncSpscConsumer<T, B, A>
where
  B: AsyncQueueBackend<T>,
  A: AsyncMutexLike<B>,
{
  pub(crate) const fn new(inner: ArcShared<A>) -> Self {
    Self { inner, _pd: PhantomData }
  }

  /// Polls the next element from the queue.
  ///
  /// # Errors
  ///
  /// Returns a `QueueError` when the backend cannot produce an element due to closure,
  /// disconnection, or backend-specific failures.
  pub async fn poll(&self) -> Result<T, QueueError<T>> {
    poll_shared::<T, B, A>(&self.inner).await
  }

  /// Signals that no more elements will be produced.
  ///
  /// # Errors
  ///
  /// Returns a `QueueError` when the backend refuses to close.
  pub async fn close(&self) -> Result<(), QueueError<T>> {
    let mut guard = <A as AsyncMutexLike<B>>::lock(&*self.inner).await.map_err(QueueError::from)?;
    guard.close().await
  }
}
