use core::marker::PhantomData;

use crate::{
  collections::queue::{QueueError, backend::SyncQueueBackend},
  sync::{ArcShared, SharedAccess, sync_mutex_like::SyncMutexLike},
};

/// Consumer for queues tagged with
/// [`SpscKey`](crate::collections::queue::type_keys::SpscKey).
pub struct SyncSpscConsumer<T, B, M>
where
  B: SyncQueueBackend<T>,
  M: SyncMutexLike<B>, {
  pub(crate) inner: ArcShared<M>,
  _pd:              PhantomData<(T, B)>,
}

impl<T, B, M> SyncSpscConsumer<T, B, M>
where
  B: SyncQueueBackend<T>,
  M: SyncMutexLike<B>,
  ArcShared<M>: SharedAccess<B>,
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
  pub fn poll(&self) -> Result<T, QueueError<T>> {
    self.inner.with_mut(|backend: &mut B| backend.poll()).map_err(QueueError::from).and_then(|result| result)
  }

  /// Signals that no more elements will be produced.
  pub fn close(&self) {
    let _ = self.inner.with_mut(|backend: &mut B| {
      backend.close();
    });
  }
}
