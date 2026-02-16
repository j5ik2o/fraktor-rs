//! Activation snapshot record stored per grain.

use alloc::{string::String, vec::Vec};

#[cfg(test)]
mod tests;

/// Captures activation state for transfer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivationRecord {
  /// Activated PID string.
  pub pid:      String,
  /// Optional snapshot bytes.
  pub snapshot: Option<Vec<u8>>,
  /// Application-level version.
  pub version:  u64,
}

impl ActivationRecord {
  /// Creates a new record.
  #[must_use]
  pub const fn new(pid: String, snapshot: Option<Vec<u8>>, version: u64) -> Self {
    Self { pid, snapshot, version }
  }
}
