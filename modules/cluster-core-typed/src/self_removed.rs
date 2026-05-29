//! Typed event emitted when the local member is removed.

use alloc::string::String;

use fraktor_cluster_core_kernel_rs::{membership::NodeStatus, topology::ClusterEvent};
use fraktor_utils_core_rs::time::TimerInstant;

/// Local-member removed event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelfRemoved {
  node_id:         String,
  authority:       String,
  previous_status: NodeStatus,
  observed_at:     TimerInstant,
}

impl SelfRemoved {
  /// Creates a local-member removed event.
  #[must_use]
  pub const fn new(node_id: String, authority: String, previous_status: NodeStatus, observed_at: TimerInstant) -> Self {
    Self { node_id, authority, previous_status, observed_at }
  }

  /// Derives [`SelfRemoved`] from a cluster member status change for the local authority.
  #[must_use]
  pub fn try_from_cluster_event(event: &ClusterEvent, self_authority: &str) -> Option<Self> {
    match event {
      | ClusterEvent::MemberStatusChanged { node_id, authority, from, to: NodeStatus::Removed, observed_at }
        if authority == self_authority =>
      {
        Some(Self::new(node_id.clone(), authority.clone(), *from, *observed_at))
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

  /// Returns the local member status before removal.
  #[must_use]
  pub const fn previous_status(&self) -> NodeStatus {
    self.previous_status
  }

  /// Returns the observation timestamp.
  #[must_use]
  pub const fn observed_at(&self) -> TimerInstant {
    self.observed_at
  }
}
