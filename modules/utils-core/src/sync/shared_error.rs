//! Errors produced while accessing shared state abstractions.

/// Errors that may arise when accessing shared backends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SharedError {
  /// Shared state has been poisoned and is no longer usable.
  Poisoned,
  /// Borrowing the shared state would result in a conflict.
  BorrowConflict,
  /// The current context does not permit blocking operations.
  InterruptContext,
}
