use core::marker::PhantomData;

use super::{AsyncQueue, async_queue_shared::offer_shared};
use crate::core::{
  collections::queue::{
    QueueError,
    backend::{AsyncQueueBackend, OfferOutcome},
    type_keys::SpscKey,
  },
  sync::{
    ArcShared,
    async_mutex_like::{AsyncMutexLike, SpinAsyncMutex},
  },
};

/// Async producer for queues tagged with
/// [`SpscKey`](crate::core::collections::queue::type_keys::SpscKey).
pub struct AsyncSpscProducerShared<T, B, A = SpinAsyncMutex<AsyncQueue<T, SpscKey, B>>>
where
  B: AsyncQueueBackend<T>,
  A: AsyncMutexLike<AsyncQueue<T, SpscKey, B>>, {
  pub(crate) inner: ArcShared<A>,
  _pd:              PhantomData<(T, B)>,
}

impl<T, B, A> AsyncSpscProducerShared<T, B, A>
where
  B: AsyncQueueBackend<T>,
  A: AsyncMutexLike<AsyncQueue<T, SpscKey, B>>,
{
  pub(crate) const fn new(inner: ArcShared<A>) -> Self {
    Self { inner, _pd: PhantomData }
  }

  /// Offers an element to the queue.
  ///
  /// # Errors
  ///
  /// Returns a `QueueError` when the backend cannot accept the element because the queue is closed,
  /// full, or disconnected.
  pub async fn offer(&self, item: T) -> Result<OfferOutcome, QueueError<T>> {
    offer_shared::<T, SpscKey, B, A>(&self.inner, item).await
  }
}
