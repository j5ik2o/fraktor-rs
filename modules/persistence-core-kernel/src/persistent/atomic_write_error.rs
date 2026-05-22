//! Atomic write construction errors.

use alloc::string::String;
use core::fmt::{Display, Formatter, Result as FmtResult};

/// Errors returned while constructing an atomic journal write.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AtomicWriteError {
  /// The atomic write payload was empty.
  Empty,
  /// The atomic write contained more than one persistence id.
  MixedPersistenceId {
    /// Expected persistence id taken from the first payload entry.
    expected: String,
    /// Actual persistence id found in a later payload entry.
    actual:   String,
  },
}

impl Display for AtomicWriteError {
  fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
    match self {
      | Self::Empty => write!(formatter, "payload must not be empty"),
      | Self::MixedPersistenceId { expected, actual } => {
        write!(formatter, "mixed persistence id: expected {expected:?}, actual {actual:?}")
      },
    }
  }
}
