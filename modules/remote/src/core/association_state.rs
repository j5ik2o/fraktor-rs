//! Describes the FSM state of a remote authority association.

use crate::core::{quarantine_reason::QuarantineReason, remote_node_id::RemoteNodeId, transport::TransportEndpoint};

/// FSM states used by the endpoint registry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssociationState {
  /// Listener registered but no handshake pending.
  Unassociated,
  /// Handshake is in progress for the provided endpoint.
  Associating {
    /// Endpoint used for the pending handshake.
    endpoint: TransportEndpoint,
  },
  /// Association is connected to the remote node id.
  Connected {
    /// Remote node information confirmed during handshake.
    remote: RemoteNodeId,
  },
  /// Authority is temporarily gated (transport should not send user traffic).
  Gated {
    /// Deadline (when provided) after which the gate may be lifted automatically.
    resume_at: Option<u64>,
  },
  /// Authority is quarantined and cannot accept traffic until manual/timeout recovery.
  Quarantined {
    /// Reason describing why the quarantine occurred.
    reason:    QuarantineReason,
    /// Optional deadline used for automatic recovery.
    resume_at: Option<u64>,
  },
}

impl AssociationState {
  /// Returns `true` when the state is connected.
  #[must_use]
  pub fn is_connected(&self) -> bool {
    matches!(self, Self::Connected { .. })
  }
}
