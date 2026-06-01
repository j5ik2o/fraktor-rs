use alloc::{string::String, vec};

use fraktor_cluster_core_kernel_rs::membership::{DataCenter, MembershipVersion, NodeRecord, NodeStatus};
use fraktor_remote_core_rs::address::{Address, UniqueAddress};

use super::GossipWireNodeRecord;

#[test]
fn to_record_preserves_app_version_roles_and_exiting_status() {
  let mut record = NodeRecord::new(
    String::from("node-1"),
    String::from("n1:4050"),
    NodeStatus::Exiting,
    MembershipVersion::new(9),
    String::from("2.0.0"),
    vec![String::from("backend"), String::from("edge")],
  );
  record.version = MembershipVersion::new(12);

  let wire = GossipWireNodeRecord::from_record(&record);
  let decoded = wire.to_record().expect("decode record");

  assert_eq!(decoded, record);
}

#[test]
fn to_record_returns_none_for_unknown_status_code() {
  let wire = GossipWireNodeRecord {
    node_id:       String::from("node-1"),
    authority:     String::from("n1:4050"),
    unique_system: String::from("cluster"),
    unique_host:   String::from("n1"),
    unique_port:   4050,
    unique_uid:    42,
    data_center:   String::from("dc-east"),
    status:        99,
    version:       1,
    join_version:  1,
    app_version:   String::from("1.0.0"),
    roles:         vec![String::from("role-a")],
  };

  assert!(wire.to_record().is_none());
}

#[test]
fn to_record_preserves_unique_address_and_data_center() {
  let identity = UniqueAddress::new(Address::new("cluster", "n1", 4050), 42);
  let data_center = DataCenter::new("dc-east");
  let record = NodeRecord::new_with_identity(
    identity.clone(),
    data_center.clone(),
    String::from("node-1"),
    NodeStatus::Up,
    MembershipVersion::new(4),
    String::from("1.0.0"),
    vec![String::from("role-a")],
  );

  let decoded = GossipWireNodeRecord::from_record(&record).to_record().expect("decode");

  assert_eq!(decoded.unique_address, identity);
  assert_eq!(decoded.data_center, data_center);
}

#[test]
fn to_record_falls_back_to_authority_system_host_and_port() {
  let wire = GossipWireNodeRecord {
    node_id:       String::from("node-1"),
    authority:     String::from("cluster@n1:4050"),
    unique_system: String::new(),
    unique_host:   String::new(),
    unique_port:   0,
    unique_uid:    42,
    data_center:   String::from("dc-east"),
    status:        1,
    version:       4,
    join_version:  4,
    app_version:   String::from("1.0.0"),
    roles:         vec![String::from("role-a")],
  };

  let decoded = wire.to_record().expect("decode");

  assert_eq!(decoded.unique_address, UniqueAddress::new(Address::new("cluster", "n1", 4050), 42));
}

#[test]
fn to_record_preserves_shutdown_related_statuses() {
  for status in [NodeStatus::PreparingForShutdown, NodeStatus::ReadyForShutdown] {
    let record = NodeRecord::new(
      String::from("node-1"),
      String::from("n1:4050"),
      status,
      MembershipVersion::new(4),
      String::from("1.0.0"),
      vec![String::from("role-a")],
    );
    let decoded = GossipWireNodeRecord::from_record(&record).to_record().expect("decode");
    assert_eq!(decoded.status, status);
  }
}

#[test]
fn to_record_preserves_weakly_up_status() {
  let record = NodeRecord::new(
    String::from("node-1"),
    String::from("n1:4050"),
    NodeStatus::WeaklyUp,
    MembershipVersion::new(4),
    String::from("1.0.0"),
    vec![String::from("role-a")],
  );

  let decoded = GossipWireNodeRecord::from_record(&record).to_record().expect("decode");

  assert_eq!(decoded.status, NodeStatus::WeaklyUp);
}
