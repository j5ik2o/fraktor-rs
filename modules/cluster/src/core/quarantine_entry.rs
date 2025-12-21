//! Quarantine entry snapshot.

use alloc::string::String;

use fraktor_utils_rs::core::time::TimerInstant;

/// Quarantine information for an authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuarantineEntry {
  /// Target authority.
  pub authority:  String,
  /// Quarantine reason.
  pub reason:     String,
  /// Expiration deadline.
  pub expires_at: TimerInstant,
}

impl QuarantineEntry {
  /// Creates a new quarantine entry.
  #[must_use]
  pub const fn new(authority: String, reason: String, expires_at: TimerInstant) -> Self {
    Self { authority, reason, expires_at }
  }
}
