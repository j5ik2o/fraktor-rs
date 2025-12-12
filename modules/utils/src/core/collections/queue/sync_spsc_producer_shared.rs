use core::marker::PhantomData;

#[cfg(test)]
mod tests;

use super::{SyncQueue, type_keys::SpscKey};
use crate::core::{
  collections::queue::{
    QueueError,
    backend::{OfferOutcome, SyncQueueBackend},
  },
  sync::{
    ArcShared,
    sync_mutex_like::{SpinSyncMutex, SyncMutexLike},
  },
};

/// Producer for queues tagged with
/// [`SpscKey`](crate::core::collections::queue::type_keys::SpscKey).
pub struct SyncSpscProducerShared<T, B, M = SpinSyncMutex<SyncQueue<T, SpscKey, B>>>
where
  B: SyncQueueBackend<T>,
  M: SyncMutexLike<SyncQueue<T, SpscKey, B>>, {
  pub(crate) inner: ArcShared<M>,
  _pd:              PhantomData<(T, B)>,
}

impl<T, B, M> Clone for SyncSpscProducerShared<T, B, M>
where
  B: SyncQueueBackend<T>,
  M: SyncMutexLike<SyncQueue<T, SpscKey, B>>,
{
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone(), _pd: PhantomData }
  }
}

impl<T, B, M> SyncSpscProducerShared<T, B, M>
where
  B: SyncQueueBackend<T>,
  M: SyncMutexLike<SyncQueue<T, SpscKey, B>>,
{
  pub(crate) const fn new(inner: ArcShared<M>) -> Self {
    Self { inner, _pd: PhantomData }
  }

  /// Offers an element to the queue.
  ///
  /// # Errors
  ///
  /// Returns a `QueueError` when the backend cannot accept the element because the queue is closed,
  /// full, or disconnected.
  pub fn offer(&self, item: T) -> Result<OfferOutcome, QueueError<T>> {
    let mut queue = self.inner.lock();
    queue.offer(item)
  }
}
