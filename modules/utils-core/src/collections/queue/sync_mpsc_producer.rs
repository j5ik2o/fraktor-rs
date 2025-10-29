use core::marker::PhantomData;

use crate::{
    collections::queue_old::QueueError,
    sync::{sync_mutex_like::SyncMutexLike, ArcShared},
};
use crate::collections::queue::backend::{OfferOutcome, SyncQueueBackend};
use crate::sync::shared_access::SharedAccess;

/// Producer for queues tagged with
/// [`MpscKey`](crate::collections::queue::type_keys::MpscKey).
pub struct SyncMpscProducer<T, B, M>
where
  B: SyncQueueBackend<T>,
  M: SyncMutexLike<B>, {
  pub(crate) inner: ArcShared<M>,
  _pd:              PhantomData<(T, B)>,
}

impl<T, B, M> SyncMpscProducer<T, B, M>
where
  B: SyncQueueBackend<T>,
  M: SyncMutexLike<B>,
  ArcShared<M>: SharedAccess<B>,
{
  pub(crate) const fn new(inner: ArcShared<M>) -> Self {
    Self { inner, _pd: PhantomData }
  }

  /// Offers an element to the queue using the underlying backend.
  ///
  /// # Errors
  ///
  /// Returns a `QueueError` when the backend cannot accept the element because the queue is closed,
  /// full, or disconnected.
  pub fn offer(&self, item: T) -> Result<OfferOutcome, QueueError<T>> {
    self.inner.with_mut(|backend: &mut B| backend.offer(item)).map_err(QueueError::from).and_then(|result| result)
  }

  /// Provides access to the shared backend.
  #[must_use]
  pub const fn shared(&self) -> &ArcShared<M> {
    &self.inner
  }
}

impl<T, B, M> Clone for SyncMpscProducer<T, B, M>
where
  B: SyncQueueBackend<T>,
  M: SyncMutexLike<B>,
  ArcShared<M>: SharedAccess<B>,
{
  fn clone(&self) -> Self {
    Self::new(self.inner.clone())
  }
}
