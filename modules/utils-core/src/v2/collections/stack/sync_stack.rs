use core::marker::PhantomData;

use crate::{
  sync::{
    ArcShared, Shared,
    sync_mutex_like::{SpinSyncMutex, SyncMutexLike},
  },
  v2::{
    collections::stack::{PushOutcome, StackBackend, StackError},
    sync::SharedAccess,
  },
};

/// Stack API parameterised by element type, backend, and shared guard.
#[derive(Clone)]
pub struct SyncStack<T, B, M = SpinSyncMutex<B>>
where
  B: StackBackend<T>,
  M: SyncMutexLike<B>, {
  inner: ArcShared<M>,
  _pd:   PhantomData<(T, B)>,
}

impl<T, B, M> SyncStack<T, B, M>
where
  B: StackBackend<T>,
  M: SyncMutexLike<B>,
  ArcShared<M>: SharedAccess<B>,
{
  /// Creates a new stack from the provided shared backend.
  #[must_use]
  pub fn new(shared_backend: ArcShared<M>) -> Self {
    Self { inner: shared_backend, _pd: PhantomData }
  }

  /// Pushes an item onto the stack.
  pub fn push(&self, item: T) -> Result<PushOutcome, StackError> {
    self.inner.with_mut(|backend: &mut B| backend.push(item)).map_err(StackError::from)?
  }

  /// Pops the top item from the stack.
  pub fn pop(&self) -> Result<T, StackError> {
    self.inner.with_mut(|backend: &mut B| backend.pop()).map_err(StackError::from)?
  }

  /// Returns the top item without removing it.
  pub fn peek(&self) -> Result<Option<T>, StackError>
  where
    T: Clone, {
    self.inner.with_mut(|backend: &mut B| Ok(backend.peek().cloned())).map_err(StackError::from)?
  }

  /// Requests the backend to transition into the closed state.
  pub fn close(&self) -> Result<(), StackError> {
    self
      .inner
      .with_mut(|backend: &mut B| {
        backend.close();
        Ok(())
      })
      .map_err(StackError::from)?
  }

  /// Returns the number of stored elements.
  #[must_use]
  pub fn len(&self) -> usize {
    self.inner.with_ref(|mutex: &M| {
      let guard = mutex.lock();
      guard.len()
    })
  }

  /// Returns the storage capacity.
  #[must_use]
  pub fn capacity(&self) -> usize {
    self.inner.with_ref(|mutex: &M| {
      let guard = mutex.lock();
      guard.capacity()
    })
  }

  /// Indicates whether the stack is empty.
  #[must_use]
  pub fn is_empty(&self) -> bool {
    self.len() == 0
  }

  /// Indicates whether the stack is full.
  #[must_use]
  pub fn is_full(&self) -> bool {
    self.len() == self.capacity()
  }

  /// Provides access to the underlying shared backend.
  #[must_use]
  pub fn shared(&self) -> &ArcShared<M> {
    &self.inner
  }
}
