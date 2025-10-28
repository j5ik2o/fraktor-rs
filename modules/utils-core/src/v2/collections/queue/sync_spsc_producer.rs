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
/// [`SpscKey`](crate::v2::collections::queue::type_keys::SpscKey).
pub struct SyncSpscProducer<T, B, M>
where
  B: SyncQueueBackend<T>,
  M: SyncMutexLike<B>, {
  pub(crate) inner: ArcShared<M>,
  _pd:              PhantomData<(T, B)>,
}

impl<T, B, M> SyncSpscProducer<T, B, M>
where
  B: SyncQueueBackend<T>,
  M: SyncMutexLike<B>,
  ArcShared<M>: SharedAccess<B>,
{
  pub(crate) fn new(inner: ArcShared<M>) -> Self {
    Self { inner, _pd: PhantomData }
  }

  /// Offers an element to the queue.
  pub fn offer(&self, item: T) -> Result<OfferOutcome, QueueError<T>> {
    let result = self.inner.with_mut(|backend: &mut B| backend.offer(item)).map_err(QueueError::from)?;
    result
  }
}
