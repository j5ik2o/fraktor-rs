use crate::collections::{QueueSize, stack::StackError};

/// Backend abstraction for stack operations.
pub trait StackBackend<T> {
  /// Pushes a value onto the stack.
  fn push(&self, value: T) -> Result<(), StackError<T>>;

  /// Pops a value from the stack.
  fn pop(&self) -> Option<T>;

  /// Clears all elements from the stack.
  fn clear(&self);

  /// Gets the current number of elements in the stack.
  fn len(&self) -> QueueSize;

  /// Gets the capacity of the stack.
  fn capacity(&self) -> QueueSize;

  /// Sets the stack's capacity.
  fn set_capacity(&self, capacity: Option<usize>);

  /// Checks if the stack is empty.
  #[must_use]
  fn is_empty(&self) -> bool {
    self.len() == QueueSize::Limited(0)
  }

  /// Gets the top value of the stack without popping.
  #[must_use]
  fn peek(&self) -> Option<T>
  where
    T: Clone;
}
