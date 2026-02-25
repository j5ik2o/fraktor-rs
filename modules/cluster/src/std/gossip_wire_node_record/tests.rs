use alloc::{string::String, vec};

use super::GossipWireNodeRecord;
use crate::core::membership::{MembershipVersion, NodeRecord, NodeStatus};

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
    node_id:      String::from("node-1"),
    authority:    String::from("n1:4050"),
    status:       99,
    version:      1,
    join_version: 1,
    app_version:  String::from("1.0.0"),
    roles:        vec![String::from("role-a")],
  };

  assert!(wire.to_record().is_none());
}
