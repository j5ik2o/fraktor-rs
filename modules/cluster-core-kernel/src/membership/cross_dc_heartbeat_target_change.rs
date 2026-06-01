//! Cross data center heartbeat target update outcome.

use alloc::vec::Vec;

use super::CrossDcHeartbeatTarget;

/// Observable target-set change after a membership update.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrossDcHeartbeatTargetChange {
  /// Newly selected cross data center targets.
  pub added:    Vec<CrossDcHeartbeatTarget>,
  /// Targets no longer selected.
  pub removed:  Vec<CrossDcHeartbeatTarget>,
  /// Targets that remain selected.
  pub retained: Vec<CrossDcHeartbeatTarget>,
}

impl CrossDcHeartbeatTargetChange {
  /// Creates a target update outcome.
  #[must_use]
  pub const fn new(
    added: Vec<CrossDcHeartbeatTarget>,
    removed: Vec<CrossDcHeartbeatTarget>,
    retained: Vec<CrossDcHeartbeatTarget>,
  ) -> Self {
    Self { added, removed, retained }
  }
}
