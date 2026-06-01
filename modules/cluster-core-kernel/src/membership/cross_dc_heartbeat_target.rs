//! Cross data center heartbeat target.

use fraktor_remote_core_rs::address::UniqueAddress;

use super::DataCenter;

/// Peer selected for cross data center heartbeat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrossDcHeartbeatTarget {
  /// Remote peer identity.
  pub peer:               UniqueAddress,
  /// Local member data center.
  pub local_data_center:  DataCenter,
  /// Remote peer data center.
  pub remote_data_center: DataCenter,
}

impl CrossDcHeartbeatTarget {
  /// Creates a cross data center heartbeat target.
  #[must_use]
  pub const fn new(peer: UniqueAddress, local_data_center: DataCenter, remote_data_center: DataCenter) -> Self {
    Self { peer, local_data_center, remote_data_center }
  }
}
