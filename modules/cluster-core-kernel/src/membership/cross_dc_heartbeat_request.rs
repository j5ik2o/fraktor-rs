//! Cross data center heartbeat request.

use super::{DataCenter, GossipPayloadKind, HeartbeatRequest};

/// Heartbeat request annotated with a cross data center pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrossDcHeartbeatRequest {
  /// Underlying heartbeat request.
  pub heartbeat:        HeartbeatRequest,
  /// Sender data center.
  pub from_data_center: DataCenter,
  /// Receiver data center.
  pub to_data_center:   DataCenter,
}

impl CrossDcHeartbeatRequest {
  /// Creates a cross data center heartbeat request.
  #[must_use]
  pub const fn new(heartbeat: HeartbeatRequest, from_data_center: DataCenter, to_data_center: DataCenter) -> Self {
    Self { heartbeat, from_data_center, to_data_center }
  }

  /// Returns the logical payload kind for transport handoff.
  #[must_use]
  pub const fn payload_kind(&self) -> GossipPayloadKind {
    GossipPayloadKind::CrossDcHeartbeat
  }
}
