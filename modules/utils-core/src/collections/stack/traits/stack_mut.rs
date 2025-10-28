use super::stack_base::StackBase;
use crate::collections::stack::StackError;

/// Mutable stack interface.
pub trait StackMut<T>: StackBase<T> {
  /// Pushes a value onto the stack.
  fn push(&mut self, value: T) -> Result<(), StackError<T>>;

  /// Pops a value from the stack.
  fn pop(&mut self) -> Option<T>;

  /// Clears all elements from the stack.
  fn clear(&mut self);

  /// Gets the top value of the stack without popping.
  #[must_use]
  fn peek(&self) -> Option<T>
  where
    T: Clone;
}
