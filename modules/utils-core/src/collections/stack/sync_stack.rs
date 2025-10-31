use core::marker::PhantomData;

use crate::{
  collections::stack::{PushOutcome, StackBackend, StackError},
  sync::{
    ArcShared, Shared, SharedAccess,
    sync_mutex_like::{SpinSyncMutex, SyncMutexLike},
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
  pub const fn new(shared_backend: ArcShared<M>) -> Self {
    Self { inner: shared_backend, _pd: PhantomData }
  }

  /// Pushes an item onto the stack.
  ///
  /// # Errors
  ///
  /// Propagates `StackError` when the backend rejects the element, for example when the stack is
  /// closed or at capacity.
  pub fn push(&self, item: T) -> Result<PushOutcome, StackError> {
    self.inner.with_mut(|backend: &mut B| backend.push(item)).map_err(StackError::from)?
  }

  /// Pops the top item from the stack.
  ///
  /// # Errors
  ///
  /// Propagates `StackError` when the backend cannot supply an element, typically due to closure
  /// or disconnection.
  pub fn pop(&self) -> Result<T, StackError> {
    self.inner.with_mut(|backend: &mut B| backend.pop()).map_err(StackError::from)?
  }

  /// Returns the top item without removing it.
  ///
  /// # Errors
  ///
  /// Propagates `StackError` when the backend cannot provide access to the top element.
  pub fn peek(&self) -> Result<Option<T>, StackError>
  where
    T: Clone, {
    self.inner.with_mut(|backend: &mut B| Ok(backend.peek().cloned())).map_err(StackError::from)?
  }

  /// Requests the backend to transition into the closed state.
  ///
  /// # Errors
  ///
  /// Propagates `StackError` when the backend refuses to close.
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
  pub const fn shared(&self) -> &ArcShared<M> {
    &self.inner
  }
}
