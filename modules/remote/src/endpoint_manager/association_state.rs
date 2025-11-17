//! Association state machine for endpoints.

use super::remote_node_id::RemoteNodeId;

/// State of an endpoint association to a remote authority.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssociationState {
  /// No handshake has been initiated yet.
  Unassociated,
  /// Handshake is in progress.
  Associating {
    /// Number of attempts performed.
    attempt: u32,
  },
  /// Connected to a remote node.
  Connected {
    /// Remote node identifier.
    remote: RemoteNodeId,
  },
}
