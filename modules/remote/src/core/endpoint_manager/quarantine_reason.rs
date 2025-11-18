//! Quarantine reason enumeration.

use alloc::string::String;

/// Describes why an authority was quarantined.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QuarantineReason {
  /// Remote UID mismatch was detected during handshake.
  UidMismatch,
  /// Manual quarantine triggered by operators.
  Manual(String),
}
