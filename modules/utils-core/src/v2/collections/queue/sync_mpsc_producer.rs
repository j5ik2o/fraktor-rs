use core::marker::PhantomData;

use crate::{
  collections::queue::QueueError,
  sync::{ArcShared, sync_mutex_like::SyncMutexLike},
  v2::{
    collections::queue::backend::{OfferOutcome, SyncQueueBackend},
    sync::SharedAccess,
  },
};

/// Producer for queues tagged with
/// [`MpscKey`](crate::v2::collections::queue::type_keys::MpscKey).
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
  pub(crate) fn new(inner: ArcShared<M>) -> Self {
    Self { inner, _pd: PhantomData }
  }

  /// Offers an element to the queue using the underlying backend.
  pub fn offer(&self, item: T) -> Result<OfferOutcome, QueueError<T>> {
    let result = self.inner.with_mut(|backend: &mut B| backend.offer(item)).map_err(QueueError::from)?;
    result
  }

  /// Provides access to the shared backend.
  #[must_use]
  pub fn shared(&self) -> &ArcShared<M> {
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
