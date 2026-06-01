//! Dedicated cluster heartbeat request.

use fraktor_remote_core_rs::address::UniqueAddress;

/// Heartbeat request addressed from one cluster member to another.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeartbeatRequest {
  /// Request sender identity.
  pub from:          UniqueAddress,
  /// Request receiver identity.
  pub to:            UniqueAddress,
  /// Peer-local heartbeat sequence number.
  pub sequence:      u64,
  /// Deadline tick for this request.
  pub deadline_tick: u64,
}

impl HeartbeatRequest {
  /// Creates a heartbeat request.
  #[must_use]
  pub const fn new(from: UniqueAddress, to: UniqueAddress, sequence: u64, deadline_tick: u64) -> Self {
    Self { from, to, sequence, deadline_tick }
  }
}
