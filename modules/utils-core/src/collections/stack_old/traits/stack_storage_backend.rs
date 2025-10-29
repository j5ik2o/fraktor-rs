use super::{stack_backend::StackBackend, stack_storage::StackStorage};
use crate::collections::{QueueSize, stack_old::StackError};

/// Backend implementation that operates directly on [`StackStorage`].
#[derive(Debug)]
pub struct StackStorageBackend<S> {
  storage: S,
}

impl<S> StackStorageBackend<S> {
  /// Creates a new `StackStorageBackend` with the specified storage.
  #[must_use]
  pub const fn new(storage: S) -> Self {
    Self { storage }
  }

  /// Gets a reference to the storage.
  #[must_use]
  pub const fn storage(&self) -> &S {
    &self.storage
  }

  /// Consumes the backend and extracts the storage.
  pub fn into_storage(self) -> S {
    self.storage
  }
}

impl<S, T> StackBackend<T> for StackStorageBackend<S>
where
  S: StackStorage<T>,
{
  fn push(&self, value: T) -> Result<(), StackError<T>> {
    self.storage().with_write(|buffer| buffer.push(value))
  }

  fn pop(&self) -> Option<T> {
    self.storage().with_write(|buffer| buffer.pop())
  }

  fn clear(&self) {
    self.storage().with_write(|buffer| buffer.clear());
  }

  fn len(&self) -> QueueSize {
    self.storage().with_read(|buffer| buffer.len())
  }

  fn capacity(&self) -> QueueSize {
    self.storage().with_read(|buffer| buffer.capacity())
  }

  fn set_capacity(&self, capacity: Option<usize>) {
    self.storage().with_write(|buffer| buffer.set_capacity(capacity));
  }

  fn peek(&self) -> Option<T>
  where
    T: Clone, {
    self.storage().with_read(|buffer| buffer.peek().cloned())
  }
}
