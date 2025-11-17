use core::marker::PhantomData;

use super::{AsyncQueue, async_queue_shared::poll_shared};
use crate::core::{
  collections::queue::{QueueError, backend::AsyncQueueBackend, type_keys::SpscKey},
  sync::{
    ArcShared,
    async_mutex_like::{AsyncMutexLike, SpinAsyncMutex},
  },
};

/// Async consumer for queues tagged with
/// [`SpscKey`](crate::core::collections::queue::type_keys::SpscKey).
pub struct AsyncSpscConsumerShared<T, B, A = SpinAsyncMutex<AsyncQueue<T, SpscKey, B>>>
where
  B: AsyncQueueBackend<T>,
  A: AsyncMutexLike<AsyncQueue<T, SpscKey, B>>, {
  pub(crate) inner: ArcShared<A>,
  _pd:              PhantomData<(T, B)>,
}

impl<T, B, A> AsyncSpscConsumerShared<T, B, A>
where
  B: AsyncQueueBackend<T>,
  A: AsyncMutexLike<AsyncQueue<T, SpscKey, B>>,
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
    poll_shared::<T, SpscKey, B, A>(&self.inner).await
  }

  /// Signals that no more elements will be produced.
  ///
  /// # Errors
  ///
  /// Returns a `QueueError` when the backend refuses to close.
  pub async fn close(&self) -> Result<(), QueueError<T>> {
    let mut guard =
      <A as AsyncMutexLike<AsyncQueue<T, SpscKey, B>>>::lock(&*self.inner).await.map_err(QueueError::from)?;
    guard.close().await
  }
}
