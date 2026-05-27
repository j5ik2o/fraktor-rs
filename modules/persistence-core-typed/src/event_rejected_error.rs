//! Event rejection error.

#[cfg(test)]
#[path = "event_rejected_error_test.rs"]
mod tests;

use core::fmt::{Display, Formatter, Result as FmtResult};

use fraktor_persistence_core_kernel_rs::error::PersistenceError;

use crate::PersistenceId;

/// Error returned when a journal rejects a persisted event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventRejectedError {
  persistence_id: PersistenceId,
  sequence_nr:    u64,
  cause:          PersistenceError,
}

impl EventRejectedError {
  /// Creates a new event rejection error.
  #[must_use]
  pub const fn new(persistence_id: PersistenceId, sequence_nr: u64, cause: PersistenceError) -> Self {
    Self { persistence_id, sequence_nr, cause }
  }

  /// Returns the persistence id whose event was rejected.
  #[must_use]
  pub const fn persistence_id(&self) -> &PersistenceId {
    &self.persistence_id
  }

  /// Returns the rejected event sequence number.
  #[must_use]
  pub const fn sequence_nr(&self) -> u64 {
    self.sequence_nr
  }

  /// Returns the rejection cause.
  #[must_use]
  pub const fn cause(&self) -> &PersistenceError {
    &self.cause
  }
}

impl Display for EventRejectedError {
  fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
    write!(
      formatter,
      "event rejected for persistence id {} at sequence number {}: {}",
      self.persistence_id.as_str(),
      self.sequence_nr,
      self.cause
    )
  }
}
