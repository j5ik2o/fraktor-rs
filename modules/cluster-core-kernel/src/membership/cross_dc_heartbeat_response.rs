//! Cross data center heartbeat response.

use super::{DataCenter, GossipPayloadKind, HeartbeatResponse};

/// Heartbeat response annotated with a cross data center pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrossDcHeartbeatResponse {
  /// Underlying heartbeat response.
  pub heartbeat:        HeartbeatResponse,
  /// Response sender data center.
  pub from_data_center: DataCenter,
  /// Response receiver data center.
  pub to_data_center:   DataCenter,
}

impl CrossDcHeartbeatResponse {
  /// Creates a cross data center heartbeat response.
  #[must_use]
  pub const fn new(heartbeat: HeartbeatResponse, from_data_center: DataCenter, to_data_center: DataCenter) -> Self {
    Self { heartbeat, from_data_center, to_data_center }
  }

  /// Returns the logical payload kind for transport handoff.
  #[must_use]
  pub const fn payload_kind(&self) -> GossipPayloadKind {
    GossipPayloadKind::CrossDcHeartbeat
  }
}
