//! Journal error types.

use alloc::string::String;
use core::fmt;

/// Errors returned by journal operations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JournalError {
  /// Storage layer reported an error.
  Storage(String),
  /// Sequence number conflict detected.
  SequenceMismatch {
    /// Expected sequence number.
    expected: u64,
    /// Actual sequence number.
    actual:   u64,
  },
}

impl fmt::Display for JournalError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      | JournalError::Storage(message) => write!(f, "journal storage error: {message}"),
      | JournalError::SequenceMismatch { expected, actual } => {
        write!(f, "journal sequence mismatch: expected {expected}, got {actual}")
      },
    }
  }
}
