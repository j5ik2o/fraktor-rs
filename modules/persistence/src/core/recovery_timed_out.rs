//! Signal emitted when recovery exceeds the configured timeout.

#[cfg(test)]
mod tests;

use alloc::string::String;

/// Signal that indicates recovery timed out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecoveryTimedOut {
  persistence_id: String,
}

impl RecoveryTimedOut {
  /// Creates a new recovery timeout signal.
  #[must_use]
  pub fn new(persistence_id: impl Into<String>) -> Self {
    Self { persistence_id: persistence_id.into() }
  }

  /// Returns the persistence id associated with the timeout.
  #[must_use]
  pub fn persistence_id(&self) -> &str {
    &self.persistence_id
  }
}
