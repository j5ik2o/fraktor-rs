//! Node record stored in the membership table.

use alloc::string::String;

use crate::core::{membership_version::MembershipVersion, node_status::NodeStatus};

/// Captures the current view of a single node in the cluster.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeRecord {
  /// Unique node identifier.
  pub node_id: String,
  /// Authority string such as `host:port`.
  pub authority: String,
  /// Current membership status.
  pub status: NodeStatus,
  /// Version the record was last updated at.
  pub version: MembershipVersion,
}

impl NodeRecord {
  /// Creates a new record with the given parameters.
  pub fn new(node_id: String, authority: String, status: NodeStatus, version: MembershipVersion) -> Self {
    Self { node_id, authority, status, version }
  }
}
