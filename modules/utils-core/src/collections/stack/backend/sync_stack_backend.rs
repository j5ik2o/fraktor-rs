use crate::collections::stack::{PushOutcome, StackError, StackOverflowPolicy, StackStorage};

/// Backend trait responsible for stack operations on top of a storage implementation.
pub trait SyncStackBackend<T> {
  /// Storage implementation backing the stack.
  type Storage: StackStorage<T>;

  /// Constructs a new backend configured with the provided storage and overflow policy.
  fn new(storage: Self::Storage, policy: StackOverflowPolicy) -> Self;

  /// Pushes an element onto the stack according to the configured overflow policy.
  ///
  /// # Errors
  ///
  /// Returns a `StackError` when the backend rejects the element because the stack is closed or the
  /// overflow policy disallows storing additional items.
  fn push(&mut self, item: T) -> Result<PushOutcome, StackError>;

  /// Pops the most recently pushed element from the stack.
  ///
  /// # Errors
  ///
  /// Returns a `StackError` when the backend cannot supply an element, typically due to closure or
  /// a storage failure.
  fn pop(&mut self) -> Result<T, StackError>;

  /// Returns a reference to the element at the top of the stack without removing it.
  fn peek(&self) -> Option<&T>;

  /// Returns the number of elements currently stored.
  fn len(&self) -> usize;

  /// Returns the maximum number of elements that can be stored without growing.
  fn capacity(&self) -> usize;

  /// Indicates whether the stack is empty.
  fn is_empty(&self) -> bool {
    self.len() == 0
  }

  /// Indicates whether the stack is full.
  fn is_full(&self) -> bool {
    self.len() == self.capacity()
  }

  /// Returns the configured overflow policy.
  fn overflow_policy(&self) -> StackOverflowPolicy;

  /// Indicates whether the backend has been closed.
  fn is_closed(&self) -> bool {
    false
  }

  /// Closes the backend, preventing further pushes while allowing remaining pops.
  fn close(&mut self) {}
}
