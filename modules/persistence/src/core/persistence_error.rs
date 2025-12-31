//! Persistence domain errors.

#[cfg(test)]
mod tests;

use alloc::string::String;
use core::fmt;

use crate::core::{journal_error::JournalError, snapshot_error::SnapshotError};

/// Errors covering persistence operations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PersistenceError {
  /// Journal operation failed.
  Journal(JournalError),
  /// Snapshot operation failed.
  Snapshot(SnapshotError),
  /// Recovery phase failed.
  Recovery(String),
  /// Persistent actor state machine failed.
  StateMachine(String),
  /// Message passing failed.
  MessagePassing(String),
}

impl fmt::Display for PersistenceError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      | PersistenceError::Journal(error) => write!(formatter, "journal error: {}", error),
      | PersistenceError::Snapshot(error) => write!(formatter, "snapshot error: {}", error),
      | PersistenceError::Recovery(reason) => write!(formatter, "recovery error: {}", reason),
      | PersistenceError::StateMachine(reason) => write!(formatter, "state machine error: {}", reason),
      | PersistenceError::MessagePassing(reason) => write!(formatter, "message passing error: {}", reason),
    }
  }
}

impl From<JournalError> for PersistenceError {
  fn from(error: JournalError) -> Self {
    Self::Journal(error)
  }
}

impl From<SnapshotError> for PersistenceError {
  fn from(error: SnapshotError) -> Self {
    Self::Snapshot(error)
  }
}
