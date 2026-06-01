//! Dedicated cluster heartbeat response.

use fraktor_remote_core_rs::address::UniqueAddress;

/// Heartbeat response preserving request identities and sequence number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeartbeatResponse {
  /// Response sender identity.
  pub from:     UniqueAddress,
  /// Response receiver identity.
  pub to:       UniqueAddress,
  /// Sequence number copied from the request.
  pub sequence: u64,
}

impl HeartbeatResponse {
  /// Creates a heartbeat response.
  #[must_use]
  pub const fn new(from: UniqueAddress, to: UniqueAddress, sequence: u64) -> Self {
    Self { from, to, sequence }
  }
}
