use core::marker::PhantomData;

use super::async_queue::offer_shared;
use crate::{
  collections::{
    queue::backend::{AsyncQueueBackend, OfferOutcome},
    queue_old::QueueError,
  },
  sync::{
    ArcShared,
    async_mutex_like::{AsyncMutexLike, SpinAsyncMutex},
  },
};

/// Async producer for queues tagged with
/// [`MpscKey`](crate::collections::queue::type_keys::MpscKey).
pub struct AsyncMpscProducer<T, B, A = SpinAsyncMutex<B>>
where
  B: AsyncQueueBackend<T>,
  A: AsyncMutexLike<B>, {
  pub(crate) inner: ArcShared<A>,
  _pd:              PhantomData<(T, B)>,
}

impl<T, B, A> AsyncMpscProducer<T, B, A>
where
  B: AsyncQueueBackend<T>,
  A: AsyncMutexLike<B>,
{
  pub(crate) const fn new(inner: ArcShared<A>) -> Self {
    Self { inner, _pd: PhantomData }
  }

  /// Offers an element to the queue using the underlying backend.
  ///
  /// # Errors
  ///
  /// Returns a `QueueError` when the backend cannot accept the element, such as when the queue is
  /// closed, full, or disconnected.
  pub async fn offer(&self, item: T) -> Result<OfferOutcome, QueueError<T>> {
    offer_shared::<T, B, A>(&self.inner, item).await
  }

  /// Provides access to the shared backend.
  #[must_use]
  pub const fn shared(&self) -> &ArcShared<A> {
    &self.inner
  }
}

impl<T, B, A> Clone for AsyncMpscProducer<T, B, A>
where
  B: AsyncQueueBackend<T>,
  A: AsyncMutexLike<B>,
{
  fn clone(&self) -> Self {
    Self::new(self.inner.clone())
  }
}
