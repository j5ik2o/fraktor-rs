//! Cross data center heartbeat evidence.

use fraktor_remote_core_rs::address::UniqueAddress;

use super::{DataCenter, GossipPayloadKind, HeartbeatEvidenceKind};

/// Liveness evidence annotated with a cross data center pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrossDcHeartbeatEvidence {
  /// Local observer identity.
  pub observer:           UniqueAddress,
  /// Remote subject identity.
  pub subject:            UniqueAddress,
  /// Observer data center.
  pub local_data_center:  DataCenter,
  /// Subject data center.
  pub remote_data_center: DataCenter,
  /// Heartbeat sequence number.
  pub sequence:           u64,
  /// Evidence category.
  pub kind:               HeartbeatEvidenceKind,
}

impl CrossDcHeartbeatEvidence {
  /// Creates cross data center heartbeat evidence.
  #[must_use]
  pub const fn new(
    observer: UniqueAddress,
    subject: UniqueAddress,
    local_data_center: DataCenter,
    remote_data_center: DataCenter,
    sequence: u64,
    kind: HeartbeatEvidenceKind,
  ) -> Self {
    Self { observer, subject, local_data_center, remote_data_center, sequence, kind }
  }

  /// Returns the logical payload kind for transport handoff.
  #[must_use]
  pub const fn payload_kind(&self) -> GossipPayloadKind {
    GossipPayloadKind::CrossDcHeartbeat
  }
}
