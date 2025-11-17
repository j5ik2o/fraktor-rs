use crate::core::collections::stack::{PushOutcome, StackError, SyncStackBackend};

/// Sync stack API parameterised by element type and backend.
pub struct SyncStack<T, B>
where
  B: SyncStackBackend<T>, {
  backend: B,
  _pd:     core::marker::PhantomData<T>,
}

impl<T, B> SyncStack<T, B>
where
  B: SyncStackBackend<T>,
{
  /// Creates a new sync stack from the provided backend.
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
  pub fn push(&mut self, item: T) -> Result<PushOutcome, StackError> {
    self.backend.push(item)
  }

  /// Pops the top item from the stack.
  ///
  /// # Errors
  ///
  /// Returns a `StackError` when the backend cannot supply an element due to closure,
  /// disconnection, or backend-specific failures.
  pub fn pop(&mut self) -> Result<T, StackError> {
    self.backend.pop()
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
  pub fn close(&mut self) {
    self.backend.close();
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

  /// Provides access to the underlying backend.
  #[must_use]
  pub const fn backend(&self) -> &B {
    &self.backend
  }
}
