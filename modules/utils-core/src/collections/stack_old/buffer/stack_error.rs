use crate::collections::QueueError;

/// Error type specific to stack operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StackError<T> {
  /// Error when attempting to add an element while the stack is at capacity.
  Full(T),
}

impl<T> From<StackError<T>> for QueueError<T> {
  fn from(err: StackError<T>) -> Self {
    match err {
      | StackError::Full(value) => QueueError::Full(value),
    }
  }
}
