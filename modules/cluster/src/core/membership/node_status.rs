//! Node status representation.

/// Represents the membership state of a cluster node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeStatus {
  /// Node requested to join and is being accepted.
  Joining,
  /// Node is active and participates in the cluster.
  Up,
  /// Node is suspected to be unreachable.
  Suspect,
  /// Node initiated a graceful leave.
  Leaving,
  /// Node has completed leave and is removed from the view.
  Removed,
  /// Node is considered dead and removed from active membership.
  Dead,
}

impl NodeStatus {
  /// Returns true when the node can serve requests.
  #[must_use]
  pub const fn is_active(self) -> bool {
    matches!(self, Self::Joining | Self::Up | Self::Suspect)
  }
}
