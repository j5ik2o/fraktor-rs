use core::marker::PhantomData;

use super::{SyncQueue, type_keys::SpscKey};
use crate::core::{
  collections::queue::{QueueError, backend::SyncQueueBackend},
  sync::{
    ArcShared, SharedAccess,
    sync_mutex_like::{SpinSyncMutex, SyncMutexLike},
  },
};

/// Consumer for queues tagged with
/// [`SpscKey`](crate::core::collections::queue::type_keys::SpscKey).
pub struct SyncSpscConsumerShared<T, B, M = SpinSyncMutex<SyncQueue<T, SpscKey, B>>>
where
  B: SyncQueueBackend<T>,
  M: SyncMutexLike<SyncQueue<T, SpscKey, B>>, {
  pub(crate) inner: ArcShared<M>,
  _pd:              PhantomData<(T, B)>,
}

impl<T, B, M> SyncSpscConsumerShared<T, B, M>
where
  B: SyncQueueBackend<T>,
  M: SyncMutexLike<SyncQueue<T, SpscKey, B>>,
  ArcShared<M>: SharedAccess<SyncQueue<T, SpscKey, B>>,
{
  pub(crate) const fn new(inner: ArcShared<M>) -> Self {
    Self { inner, _pd: PhantomData }
  }

  /// Polls the next element from the queue.
  ///
  /// # Errors
  ///
  /// Returns a `QueueError` when the backend cannot produce an element due to closure,
  /// disconnection, or backend-specific failures.
  pub fn poll(&mut self) -> Result<T, QueueError<T>> {
    self.inner.with_mut(|queue: &mut SyncQueue<T, SpscKey, B>| queue.poll()).map_err(QueueError::from)?
  }

  /// Signals that no more elements will be produced.
  ///
  /// # Errors
  ///
  /// Returns a `QueueError` when the backend refuses to close.
  pub fn close(&mut self) -> Result<(), QueueError<T>> {
    self.inner.with_mut(|queue: &mut SyncQueue<T, SpscKey, B>| queue.close()).map_err(QueueError::from)?
  }
}
