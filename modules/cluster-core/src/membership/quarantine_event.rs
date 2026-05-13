//! Quarantine events emitted by the coordinator.

use alloc::string::String;

/// Quarantine event emitted when entries are added or cleared.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum QuarantineEvent {
  /// Authority was quarantined.
  Quarantined {
    /// Target authority.
    authority: String,
    /// Quarantine reason.
    reason:    String,
  },
  /// Authority quarantine was cleared.
  Cleared {
    /// Target authority.
    authority: String,
  },
}
