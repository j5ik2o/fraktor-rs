use alloc::{string::String, vec};

use fraktor_remote_core_rs::address::{Address, UniqueAddress};

use crate::membership::{
  DataCenter, MembershipSnapshot, MembershipVersion, NodeRecord, NodeStatus, ReachabilityMatrix,
};

#[test]
fn members_in_data_center_preserves_identity_status_and_roles() {
  let east = DataCenter::new("dc-east");
  let west = DataCenter::new("dc-west");
  let east_member = NodeRecord::new_with_identity(
    UniqueAddress::new(Address::new("cluster", "n1", 4050), 10),
    east.clone(),
    String::from("node-1"),
    NodeStatus::Suspect,
    MembershipVersion::new(1),
    String::from("1.0.0"),
    vec![String::from("backend")],
  );
  let west_member = NodeRecord::new_with_identity(
    UniqueAddress::new(Address::new("cluster", "n2", 4050), 11),
    west.clone(),
    String::from("node-2"),
    NodeStatus::Up,
    MembershipVersion::new(2),
    String::from("1.0.0"),
    vec![String::from("frontend")],
  );
  let snapshot = MembershipSnapshot::new(MembershipVersion::new(2), vec![east_member.clone(), west_member]);

  assert_eq!(snapshot.members_in_data_center(&east), vec![east_member]);
}

#[test]
fn membership_snapshot_keeps_reachability_snapshot() {
  let observer = UniqueAddress::new(Address::new("cluster", "observer", 4050), 10);
  let subject = UniqueAddress::new(Address::new("cluster", "subject", 4050), 11);
  let data_center = DataCenter::new("dc-east");
  let member = NodeRecord::new_with_identity(
    subject.clone(),
    data_center.clone(),
    String::from("node-1"),
    NodeStatus::WeaklyUp,
    MembershipVersion::new(1),
    String::from("1.0.0"),
    vec![String::from("backend")],
  );
  let mut reachability = ReachabilityMatrix::new();
  reachability.unreachable(observer, subject.clone());

  let snapshot =
    MembershipSnapshot::new_with_reachability(MembershipVersion::new(1), vec![member], reachability.snapshot());

  assert_eq!(snapshot.entries[0].unique_address, subject);
  assert_eq!(snapshot.entries[0].data_center, data_center);
  assert_eq!(snapshot.entries[0].status, NodeStatus::WeaklyUp);
  assert_eq!(snapshot.reachability.aggregate_status(&subject), crate::membership::ReachabilityStatus::Unreachable);
}
