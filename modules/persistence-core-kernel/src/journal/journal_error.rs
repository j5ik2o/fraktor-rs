//! Journal operation errors.

#[cfg(test)]
#[path = "journal_error_test.rs"]
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
  /// Atomic write payload failed local validation before it reached the journal.
  InvalidAtomicWrite(String),
  /// A write batch contained multiple persistence ids.
  MixedPersistenceId {
    /// Expected persistence id for the batch.
    expected: String,
    /// Actual persistence id encountered.
    actual:   String,
  },
  /// Backend cannot guarantee a multi-entry atomic write.
  UnsupportedAtomicWrite {
    /// Number of events in the rejected atomic write.
    size: usize,
  },
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
      | JournalError::InvalidAtomicWrite(reason) => write!(formatter, "invalid atomic write: {}", reason),
      | JournalError::MixedPersistenceId { expected, actual } => {
        write!(formatter, "mixed persistence id: expected {}, actual {}", expected, actual)
      },
      | JournalError::UnsupportedAtomicWrite { size } => {
        write!(formatter, "unsupported atomic write size: {}", size)
      },
      | JournalError::ReadFailed(reason) => write!(formatter, "read failed: {}", reason),
      | JournalError::DeleteFailed(reason) => write!(formatter, "delete failed: {}", reason),
    }
  }
}
