use core::marker::PhantomData;

use super::{SyncQueue, type_keys::MpscKey};
use crate::{
  collections::queue::{
    QueueError,
    backend::{OfferOutcome, SyncQueueBackend},
  },
  sync::{
    ArcShared, SharedAccess,
    sync_mutex_like::{SpinSyncMutex, SyncMutexLike},
  },
};

/// Producer for queues tagged with
/// [`MpscKey`](crate::collections::queue::type_keys::MpscKey).
pub struct SyncMpscProducerShared<T, B, M = SpinSyncMutex<SyncQueue<T, MpscKey, B>>>
where
  B: SyncQueueBackend<T>,
  M: SyncMutexLike<SyncQueue<T, MpscKey, B>>, {
  pub(crate) inner: ArcShared<M>,
  _pd:              PhantomData<(T, B)>,
}

impl<T, B, M> SyncMpscProducerShared<T, B, M>
where
  B: SyncQueueBackend<T>,
  M: SyncMutexLike<SyncQueue<T, MpscKey, B>>,
  ArcShared<M>: SharedAccess<SyncQueue<T, MpscKey, B>>,
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
    self.inner.with_mut(|queue: &mut SyncQueue<T, MpscKey, B>| queue.offer(item)).map_err(QueueError::from)?
  }

  /// Provides access to the shared backend.
  #[must_use]
  pub const fn shared(&self) -> &ArcShared<M> {
    &self.inner
  }
}

impl<T, B, M> Clone for SyncMpscProducerShared<T, B, M>
where
  B: SyncQueueBackend<T>,
  M: SyncMutexLike<SyncQueue<T, MpscKey, B>>,
  ArcShared<M>: SharedAccess<SyncQueue<T, MpscKey, B>>,
{
  fn clone(&self) -> Self {
    Self::new(self.inner.clone())
  }
}
