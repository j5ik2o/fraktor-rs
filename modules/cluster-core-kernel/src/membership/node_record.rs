//! Node record stored in the membership table.

#[cfg(test)]
#[path = "node_record_test.rs"]
mod tests;

use alloc::{
  string::{String, ToString},
  vec::Vec,
};

use fraktor_remote_core_rs::address::{Address, UniqueAddress};

use super::{DataCenter, MembershipVersion, NodeStatus};

/// Captures the current view of a single node in the cluster.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeRecord {
  /// Unique remote address identifying this node incarnation.
  pub unique_address: UniqueAddress,
  /// Data center this member belongs to.
  pub data_center:    DataCenter,
  /// Unique node identifier.
  pub node_id:        String,
  /// Authority string such as `host:port`.
  pub authority:      String,
  /// Current membership status.
  pub status:         NodeStatus,
  /// Version the record was last updated at.
  pub version:        MembershipVersion,
  /// Version when the node joined most recently.
  pub join_version:   MembershipVersion,
  /// Application version advertised by the node.
  pub app_version:    String,
  /// Roles assigned to the node.
  pub roles:          Vec<String>,
}

impl NodeRecord {
  /// Creates a new record with the given parameters.
  #[must_use]
  pub fn new(
    node_id: String,
    authority: String,
    status: NodeStatus,
    version: MembershipVersion,
    app_version: String,
    roles: Vec<String>,
  ) -> Self {
    Self {
      unique_address: unique_address_from_authority(authority.clone()),
      data_center: DataCenter::default(),
      node_id,
      authority,
      status,
      version,
      join_version: version,
      app_version,
      roles,
    }
  }

  /// Creates a new record with an explicit unique address and data center.
  #[must_use]
  pub fn new_with_identity(
    unique_address: UniqueAddress,
    data_center: DataCenter,
    node_id: String,
    status: NodeStatus,
    version: MembershipVersion,
    app_version: String,
    roles: Vec<String>,
  ) -> Self {
    let authority = unique_address.address().to_string();
    Self { unique_address, data_center, node_id, authority, status, version, join_version: version, app_version, roles }
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

fn unique_address_from_authority(authority: String) -> UniqueAddress {
  let (host, port) = authority_host_port(authority);
  UniqueAddress::new(Address::new("fraktor-cluster", host, port), 1)
}

fn authority_host_port(authority: String) -> (String, u16) {
  if let Some((host, port_text)) = authority.rsplit_once(':')
    && let Ok(port) = port_text.parse::<u16>()
  {
    (host.to_string(), port)
  } else {
    (authority, 0)
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
