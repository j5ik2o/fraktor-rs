use core::marker::PhantomData;

use crate::{
  collections::queue_old::QueueError,
  sync::{sync_mutex_like::SyncMutexLike, ArcShared, Shared},
};
use crate::collections::queue::backend::SyncQueueBackend;
use crate::sync::shared_access::SharedAccess;

/// Consumer for queues tagged with
/// [`MpscKey`](crate::collections::queue::type_keys::MpscKey).
pub struct SyncMpscConsumer<T, B, M>
where
  B: SyncQueueBackend<T>,
  M: SyncMutexLike<B>, {
  pub(crate) inner: ArcShared<M>,
  _pd:              PhantomData<(T, B)>,
}

impl<T, B, M> SyncMpscConsumer<T, B, M>
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

  /// Returns the number of stored elements.
  #[must_use]
  pub fn len(&self) -> usize {
    self.inner.with_ref(|mutex: &M| {
      let guard = mutex.lock();
      guard.len()
    })
  }

  /// Returns the queue capacity.
  #[must_use]
  pub fn capacity(&self) -> usize {
    self.inner.with_ref(|mutex: &M| {
      let guard = mutex.lock();
      guard.capacity()
    })
  }

  /// Indicates whether the queue is empty.
  #[must_use]
  pub fn is_empty(&self) -> bool {
    self.len() == 0
  }
}
