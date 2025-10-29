use crate::collections::QueueSize;

/// Base trait for stack-like collections.
pub trait StackBase<T> {
  /// Gets the current number of elements in the stack.
  fn len(&self) -> QueueSize;

  /// Gets the capacity of the stack.
  fn capacity(&self) -> QueueSize;

  /// Checks if the stack is empty.
  #[must_use]
  fn is_empty(&self) -> bool {
    self.len().to_usize() == 0
  }
}
