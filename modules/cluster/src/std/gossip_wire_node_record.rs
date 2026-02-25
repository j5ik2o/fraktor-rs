//! Wire representation of a membership node record.

#[cfg(test)]
mod tests;

use alloc::{string::String, vec::Vec};

use serde::{Deserialize, Serialize};

use crate::core::membership::{MembershipVersion, NodeRecord, NodeStatus};

/// Wire representation of a node record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct GossipWireNodeRecord {
  /// Unique node identifier.
  pub node_id:      String,
  /// Authority string such as `host:port`.
  pub authority:    String,
  /// Node status encoded as u8.
  pub status:       u8,
  /// Membership version.
  pub version:      u64,
  /// Join version used for age ordering.
  pub join_version: u64,
  /// Application version.
  pub app_version:  String,
  /// Roles assigned to the node.
  pub roles:        Vec<String>,
}

impl GossipWireNodeRecord {
  pub(crate) fn from_record(record: &NodeRecord) -> Self {
    Self {
      node_id:      record.node_id.clone(),
      authority:    record.authority.clone(),
      status:       status_to_u8(record.status),
      version:      record.version.value(),
      join_version: record.join_version.value(),
      app_version:  record.app_version.clone(),
      roles:        record.roles.clone(),
    }
  }

  pub(crate) fn to_record(&self) -> Option<NodeRecord> {
    let status = status_from_u8(self.status)?;
    Some(NodeRecord {
      node_id: self.node_id.clone(),
      authority: self.authority.clone(),
      status,
      version: MembershipVersion::new(self.version),
      join_version: MembershipVersion::new(self.join_version),
      app_version: self.app_version.clone(),
      roles: self.roles.clone(),
    })
  }
}

fn status_to_u8(status: NodeStatus) -> u8 {
  match status {
    | NodeStatus::Joining => 0,
    | NodeStatus::Up => 1,
    | NodeStatus::Suspect => 2,
    | NodeStatus::Leaving => 3,
    | NodeStatus::Removed => 4,
    | NodeStatus::Dead => 5,
    | NodeStatus::Exiting => 6,
  }
}

fn status_from_u8(value: u8) -> Option<NodeStatus> {
  match value {
    | 0 => Some(NodeStatus::Joining),
    | 1 => Some(NodeStatus::Up),
    | 2 => Some(NodeStatus::Suspect),
    | 3 => Some(NodeStatus::Leaving),
    | 4 => Some(NodeStatus::Removed),
    | 5 => Some(NodeStatus::Dead),
    | 6 => Some(NodeStatus::Exiting),
    | _ => None,
  }
}
