use core::marker::PhantomData;

use super::async_queue_shared::offer_shared;
use crate::{
  collections::queue::{
    QueueError,
    backend::{AsyncQueueBackend, OfferOutcome},
  },
  sync::{
    ArcShared,
    async_mutex_like::{AsyncMutexLike, SpinAsyncMutex},
  },
};

/// Async producer for queues tagged with
/// [`SpscKey`](crate::collections::queue::type_keys::SpscKey).
pub struct AsyncSpscProducerShared<T, B, A = SpinAsyncMutex<B>>
where
  B: AsyncQueueBackend<T>,
  A: AsyncMutexLike<B>, {
  pub(crate) inner: ArcShared<A>,
  _pd:              PhantomData<(T, B)>,
}

impl<T, B, A> AsyncSpscProducerShared<T, B, A>
where
  B: AsyncQueueBackend<T>,
  A: AsyncMutexLike<B>,
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
    offer_shared::<T, B, A>(&self.inner, item).await
  }
}
