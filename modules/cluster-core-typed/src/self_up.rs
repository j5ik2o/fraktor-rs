//! Typed event emitted when the local member becomes up.

use alloc::string::String;

use fraktor_cluster_core_kernel_rs::{
  membership::{CurrentClusterState, NodeStatus},
  topology::ClusterEvent,
};

/// Local-member up event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelfUp {
  node_id:               String,
  authority:             String,
  current_cluster_state: CurrentClusterState,
}

impl SelfUp {
  /// Creates a local-member up event.
  #[must_use]
  pub const fn new(node_id: String, authority: String, current_cluster_state: CurrentClusterState) -> Self {
    Self { node_id, authority, current_cluster_state }
  }

  /// Derives [`SelfUp`] from a cluster member status change for the local authority.
  #[must_use]
  pub fn try_from_cluster_event(
    event: &ClusterEvent,
    self_authority: &str,
    current_cluster_state: CurrentClusterState,
  ) -> Option<Self> {
    match event {
      | ClusterEvent::MemberStatusChanged { node_id, authority, to: NodeStatus::Up, .. }
        if authority == self_authority =>
      {
        Some(Self::new(node_id.clone(), authority.clone(), current_cluster_state))
      },
      | _ => None,
    }
  }

  /// Returns the local node identifier.
  #[must_use]
  pub fn node_id(&self) -> &str {
    &self.node_id
  }

  /// Returns the local member authority.
  #[must_use]
  pub fn authority(&self) -> &str {
    &self.authority
  }

  /// Returns the current cluster state observed when this event was emitted.
  #[must_use]
  pub const fn current_cluster_state(&self) -> &CurrentClusterState {
    &self.current_cluster_state
  }
}
