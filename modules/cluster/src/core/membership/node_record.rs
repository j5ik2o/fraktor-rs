//! Node record stored in the membership table.

#[cfg(test)]
mod tests;

use alloc::{string::String, vec::Vec};

use super::{MembershipVersion, NodeStatus};

/// Captures the current view of a single node in the cluster.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeRecord {
  /// Unique node identifier.
  pub node_id:      String,
  /// Authority string such as `host:port`.
  pub authority:    String,
  /// Current membership status.
  pub status:       NodeStatus,
  /// Version the record was last updated at.
  pub version:      MembershipVersion,
  /// Version when the node joined most recently.
  pub join_version: MembershipVersion,
  /// Application version advertised by the node.
  pub app_version:  String,
  /// Roles assigned to the node.
  pub roles:        Vec<String>,
}

impl NodeRecord {
  /// Creates a new record with the given parameters.
  #[must_use]
  pub const fn new(
    node_id: String,
    authority: String,
    status: NodeStatus,
    version: MembershipVersion,
    app_version: String,
    roles: Vec<String>,
  ) -> Self {
    Self { node_id, authority, status, version, join_version: version, app_version, roles }
  }

  /// Returns true if this record is older than `other` by join version.
  #[must_use]
  pub fn is_older_than(&self, other: &Self) -> bool {
    if self.join_version == other.join_version {
      authority_order_key(self.authority.as_str()) < authority_order_key(other.authority.as_str())
    } else {
      self.join_version < other.join_version
    }
  }
}

fn authority_order_key(authority: &str) -> (&str, u32) {
  if let Some((host, port_text)) = authority.rsplit_once(':')
    && let Ok(port) = port_text.parse::<u32>()
  {
    (host, port)
  } else {
    (authority, 0)
  }
}
