use crate::collections::{
  stack::{AsyncStackBackend, PushOutcome, StackError},
  wait::{WaitError, WaitShared},
};

/// Async stack API parameterised by element type and backend.
#[derive(Clone)]
pub struct AsyncStack<T, B>
where
  B: AsyncStackBackend<T>, {
  backend: B,
  _pd:     core::marker::PhantomData<T>,
}

impl<T, B> AsyncStack<T, B>
where
  B: AsyncStackBackend<T>,
{
  /// Creates a new async stack from the provided backend.
  #[must_use]
  pub const fn new(backend: B) -> Self {
    Self { backend, _pd: core::marker::PhantomData }
  }

  /// Pushes an item onto the stack according to the backend's overflow policy.
  ///
  /// # Errors
  ///
  /// Returns a `StackError` when the backend rejects the element because the stack is closed,
  /// full, or disconnected.
  pub async fn push(&mut self, item: T) -> Result<PushOutcome, StackError> {
    self.backend.push(item).await
  }

  /// Pops the top item from the stack.
  ///
  /// # Errors
  ///
  /// Returns a `StackError` when the backend cannot supply an element due to closure,
  /// disconnection, or backend-specific failures.
  pub async fn pop(&mut self) -> Result<T, StackError> {
    self.backend.pop().await
  }

  /// Returns the top item without removing it.
  ///
  /// # Errors
  ///
  /// Returns a `StackError` when the backend cannot access the top element due to closure,
  /// disconnection, or backend-specific failures.
  pub fn peek(&self) -> Result<Option<T>, StackError>
  where
    T: Clone, {
    Ok(self.backend.peek().cloned())
  }

  /// Requests the backend to transition into the closed state.
  pub async fn close(&mut self) {
    let _ = self.backend.close().await;
  }

  /// Returns the current number of stored elements.
  #[must_use]
  pub fn len(&self) -> usize {
    self.backend.len()
  }

  /// Returns the storage capacity.
  #[must_use]
  pub fn capacity(&self) -> usize {
    self.backend.capacity()
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

  /// Indicates whether the stack is closed.
  #[must_use]
  pub fn is_closed(&self) -> bool {
    self.backend.is_closed()
  }

  /// Prepares a wait for push availability.
  ///
  /// # Errors
  ///
  /// Returns a `WaitError` when the waiter cannot be registered.
  pub fn prepare_push_wait(&mut self) -> Result<Option<WaitShared<StackError>>, WaitError> {
    self.backend.prepare_push_wait()
  }

  /// Prepares a wait for pop availability.
  ///
  /// # Errors
  ///
  /// Returns a `WaitError` when the waiter cannot be registered.
  pub fn prepare_pop_wait(&mut self) -> Result<Option<WaitShared<StackError>>, WaitError> {
    self.backend.prepare_pop_wait()
  }

  /// Provides access to the underlying backend.
  #[must_use]
  pub const fn backend(&self) -> &B {
    &self.backend
  }
}
