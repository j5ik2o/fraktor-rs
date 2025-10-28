use core::marker::PhantomData;

use crate::{
  collections::queue::QueueError,
  sync::{ArcShared, Shared, sync_mutex_like::SyncMutexLike},
  v2::{collections::queue::backend::SyncQueueBackend, sync::SharedAccess},
};

/// Consumer for queues tagged with
/// [`MpscKey`](crate::v2::collections::queue::type_keys::MpscKey).
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
  pub(crate) fn new(inner: ArcShared<M>) -> Self {
    Self { inner, _pd: PhantomData }
  }

  /// Polls the next element from the queue.
  pub fn poll(&self) -> Result<T, QueueError<T>> {
    let result = self.inner.with_mut(|backend: &mut B| backend.poll()).map_err(QueueError::from)?;
    result
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
