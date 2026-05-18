//! Persistence domain errors.

#[cfg(test)]
#[path = "persistence_error_test.rs"]
mod tests;

use alloc::string::String;
use core::fmt::{Display, Formatter, Result as FmtResult};

use crate::{journal::JournalError, snapshot::SnapshotError};

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

impl Display for PersistenceError {
  fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
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
