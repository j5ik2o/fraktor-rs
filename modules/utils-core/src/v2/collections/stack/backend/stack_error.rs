use crate::v2::sync::SharedError;

/// Errors that may arise while operating on a stack backend.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StackError {
  /// The stack contains no elements.
  Empty,
  /// The stack has been closed and will not accept further operations.
  Closed,
  /// The underlying shared state is no longer accessible.
  Disconnected,
  /// The operation would block and cannot proceed in the current context.
  WouldBlock,
  /// Allocator-related failure occurred while growing the storage.
  AllocError,
  /// The stack cannot accept new elements.
  Full,
}

impl From<SharedError> for StackError {
  fn from(err: SharedError) -> Self {
    match err {
      | SharedError::Poisoned => StackError::Disconnected,
      | SharedError::BorrowConflict => StackError::WouldBlock,
      | SharedError::InterruptContext => StackError::WouldBlock,
    }
  }
}
