//! Wire representation of a membership node record.

use alloc::{
  string::{String, ToString},
  vec::Vec,
};

use fraktor_cluster_core_kernel_rs::membership::{DataCenter, MembershipVersion, NodeRecord, NodeStatus};
use fraktor_remote_core_rs::address::{Address, UniqueAddress};
use serde::{Deserialize, Serialize};

#[cfg(test)]
#[path = "gossip_wire_node_record_test.rs"]
mod tests;

/// Wire representation of a node record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct GossipWireNodeRecord {
  /// Unique node identifier.
  pub node_id:       String,
  /// Authority string such as `host:port`.
  pub authority:     String,
  /// Actor system name from the unique address.
  #[serde(default)]
  pub unique_system: String,
  /// Host from the unique address.
  #[serde(default)]
  pub unique_host:   String,
  /// Port from the unique address.
  #[serde(default)]
  pub unique_port:   u16,
  /// UID from the unique address.
  #[serde(default = "default_unique_uid")]
  pub unique_uid:    u64,
  /// Data center name.
  #[serde(default = "default_data_center_name")]
  pub data_center:   String,
  /// Node status encoded as u8.
  pub status:        u8,
  /// Membership version.
  pub version:       u64,
  /// Join version used for age ordering.
  pub join_version:  u64,
  /// Application version.
  pub app_version:   String,
  /// Roles assigned to the node.
  pub roles:         Vec<String>,
}

impl GossipWireNodeRecord {
  pub(crate) fn from_record(record: &NodeRecord) -> Self {
    Self {
      node_id:       record.node_id.clone(),
      authority:     record.authority.clone(),
      unique_system: record.unique_address.address().system().to_string(),
      unique_host:   record.unique_address.address().host().to_string(),
      unique_port:   record.unique_address.address().port(),
      unique_uid:    record.unique_address.uid(),
      data_center:   record.data_center.as_str().to_string(),
      status:        status_to_u8(record.status),
      version:       record.version.value(),
      join_version:  record.join_version.value(),
      app_version:   record.app_version.clone(),
      roles:         record.roles.clone(),
    }
  }

  pub(crate) fn to_record(&self) -> Option<NodeRecord> {
    let status = status_from_u8(self.status)?;
    let unique_address = self.unique_address();
    let data_center = self.data_center();
    let mut record = NodeRecord::new_with_identity(
      unique_address,
      data_center,
      self.node_id.clone(),
      status,
      MembershipVersion::new(self.version),
      self.app_version.clone(),
      self.roles.clone(),
    );
    record.authority = self.authority.clone();
    record.join_version = MembershipVersion::new(self.join_version);
    Some(record)
  }

  fn unique_address(&self) -> UniqueAddress {
    let (host, port) = if self.unique_host.is_empty() {
      authority_host_port(self.authority.clone())
    } else {
      (self.unique_host.clone(), self.unique_port)
    };
    let system = if self.unique_system.is_empty() { "fraktor-cluster".to_string() } else { self.unique_system.clone() };
    UniqueAddress::new(Address::new(system, host, port), self.unique_uid)
  }

  fn data_center(&self) -> DataCenter {
    if self.data_center.is_empty() { DataCenter::default() } else { DataCenter::new(self.data_center.clone()) }
  }
}

fn default_unique_uid() -> u64 {
  1
}

fn default_data_center_name() -> String {
  "default".to_string()
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

fn status_to_u8(status: NodeStatus) -> u8 {
  match status {
    | NodeStatus::Joining => 0,
    | NodeStatus::Up => 1,
    | NodeStatus::Suspect => 2,
    | NodeStatus::Leaving => 3,
    | NodeStatus::Removed => 4,
    | NodeStatus::Dead => 5,
    | NodeStatus::Exiting => 6,
    | NodeStatus::PreparingForShutdown => 7,
    | NodeStatus::ReadyForShutdown => 8,
    | NodeStatus::WeaklyUp => 9,
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
    | 7 => Some(NodeStatus::PreparingForShutdown),
    | 8 => Some(NodeStatus::ReadyForShutdown),
    | 9 => Some(NodeStatus::WeaklyUp),
    | _ => None,
  }
}
