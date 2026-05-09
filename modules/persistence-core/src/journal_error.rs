//! Journal operation errors.

#[cfg(test)]
mod tests;

use alloc::string::String;
use core::fmt::{Display, Formatter, Result as FmtResult};

/// Errors returned by journal operations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JournalError {
  /// Sequence number mismatch detected while appending events.
  SequenceMismatch {
    /// Expected next sequence number.
    expected: u64,
    /// Actual sequence number encountered.
    actual:   u64,
  },
  /// Failed to write messages.
  WriteFailed(String),
  /// Failed to read messages.
  ReadFailed(String),
  /// Failed to delete messages.
  DeleteFailed(String),
}

impl Display for JournalError {
  fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
    match self {
      | JournalError::SequenceMismatch { expected, actual } => {
        write!(formatter, "sequence mismatch: expected {}, actual {}", expected, actual)
      },
      | JournalError::WriteFailed(reason) => write!(formatter, "write failed: {}", reason),
      | JournalError::ReadFailed(reason) => write!(formatter, "read failed: {}", reason),
      | JournalError::DeleteFailed(reason) => write!(formatter, "delete failed: {}", reason),
    }
  }
}
