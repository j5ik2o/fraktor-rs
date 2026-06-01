//! Current cluster state snapshot used by cluster events.

#[cfg(test)]
#[path = "current_cluster_state_test.rs"]
mod tests;

use alloc::{collections::BTreeMap, string::String, vec::Vec};

use crate::membership::{DataCenter, NodeRecord, ReachabilitySnapshot};

/// Enriched cluster state containing leaders, seen members, and unreachable members.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentClusterState {
  /// Members currently considered part of the cluster view.
  pub members:      Vec<NodeRecord>,
  /// Members currently considered unreachable.
  pub unreachable:  Vec<NodeRecord>,
  /// Authorities that have seen the latest gossip version.
  pub seen_by:      Vec<String>,
  /// Current oldest-member leader authority.
  pub leader:       Option<String>,
  /// Role-specific leaders (`role -> authority?`).
  pub role_leader:  BTreeMap<String, Option<String>>,
  /// Reachability evidence known when the state was emitted.
  pub reachability: ReachabilitySnapshot,
}

impl CurrentClusterState {
  /// Creates an enriched cluster state.
  #[must_use]
  pub const fn new(
    members: Vec<NodeRecord>,
    unreachable: Vec<NodeRecord>,
    seen_by: Vec<String>,
    leader: Option<String>,
    role_leader: BTreeMap<String, Option<String>>,
  ) -> Self {
    Self { members, unreachable, seen_by, leader, role_leader, reachability: ReachabilitySnapshot::empty() }
  }

  /// Creates an enriched cluster state with reachability evidence.
  #[must_use]
  pub const fn new_with_reachability(
    members: Vec<NodeRecord>,
    unreachable: Vec<NodeRecord>,
    seen_by: Vec<String>,
    leader: Option<String>,
    role_leader: BTreeMap<String, Option<String>>,
    reachability: ReachabilitySnapshot,
  ) -> Self {
    Self { members, unreachable, seen_by, leader, role_leader, reachability }
  }

  /// Returns members in the requested data center.
  #[must_use]
  pub fn members_in_data_center(&self, data_center: &DataCenter) -> Vec<NodeRecord> {
    self.members.iter().filter(|record| &record.data_center == data_center).cloned().collect()
  }

  /// Returns unreachable members in the requested data center.
  #[must_use]
  pub fn unreachable_in_data_center(&self, data_center: &DataCenter) -> Vec<NodeRecord> {
    self.unreachable.iter().filter(|record| &record.data_center == data_center).cloned().collect()
  }
}
