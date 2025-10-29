use core::marker::PhantomData;

use super::async_queue::offer_shared;
use crate::{
    collections::queue_old::QueueError,
    sync::{
        async_mutex_like::{AsyncMutexLike, SpinAsyncMutex},
        ArcShared,
    },
};
use crate::collections::queue::backend::{AsyncQueueBackend, OfferOutcome};

/// Async producer for queues tagged with
/// [`SpscKey`](crate::collections::queue::type_keys::SpscKey).
pub struct AsyncSpscProducer<T, B, A = SpinAsyncMutex<B>>
where
  B: AsyncQueueBackend<T>,
  A: AsyncMutexLike<B>, {
  pub(crate) inner: ArcShared<A>,
  _pd:              PhantomData<(T, B)>,
}

impl<T, B, A> AsyncSpscProducer<T, B, A>
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
