//! Current cluster state snapshot used by cluster events.

#[cfg(test)]
mod tests;

use alloc::{collections::BTreeMap, string::String, vec::Vec};

use crate::core::membership::NodeRecord;

/// Enriched cluster state containing leaders, seen members, and unreachable members.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentClusterState {
  /// Members currently considered part of the cluster view.
  pub members:     Vec<NodeRecord>,
  /// Members currently considered unreachable.
  pub unreachable: Vec<NodeRecord>,
  /// Authorities that have seen the latest gossip version.
  pub seen_by:     Vec<String>,
  /// Current oldest-member leader authority.
  pub leader:      Option<String>,
  /// Role-specific leaders (`role -> authority?`).
  pub role_leader: BTreeMap<String, Option<String>>,
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
    Self { members, unreachable, seen_by, leader, role_leader }
  }
}
