//! Heartbeat evidence for reachability input.

use fraktor_remote_core_rs::address::UniqueAddress;

use super::HeartbeatEvidenceKind;

/// Dedicated heartbeat output that can feed reachability without deciding downing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeartbeatEvidence {
  /// Local observer identity.
  pub observer: UniqueAddress,
  /// Remote subject identity.
  pub subject:  UniqueAddress,
  /// Sequence number the evidence refers to.
  pub sequence: u64,
  /// Evidence category.
  pub kind:     HeartbeatEvidenceKind,
}

impl HeartbeatEvidence {
  /// Creates heartbeat evidence.
  #[must_use]
  pub const fn new(
    observer: UniqueAddress,
    subject: UniqueAddress,
    sequence: u64,
    kind: HeartbeatEvidenceKind,
  ) -> Self {
    Self { observer, subject, sequence, kind }
  }
}
