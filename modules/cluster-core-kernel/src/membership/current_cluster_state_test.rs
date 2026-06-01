use alloc::{collections::BTreeMap, string::String, vec};

use fraktor_remote_core_rs::address::{Address, UniqueAddress};

use crate::membership::{CurrentClusterState, DataCenter, MembershipVersion, NodeRecord, NodeStatus};

#[test]
fn current_cluster_state_keeps_leader_and_unreachable_fields() {
  let member = NodeRecord::new(
    String::from("node-1"),
    String::from("node-a:2552"),
    NodeStatus::Up,
    MembershipVersion::new(1),
    String::from("1.0.0"),
    vec![String::from("backend")],
  );
  let unreachable = NodeRecord::new(
    String::from("node-2"),
    String::from("node-b:2552"),
    NodeStatus::Suspect,
    MembershipVersion::new(2),
    String::from("1.0.0"),
    vec![String::from("backend")],
  );

  let mut role_leader = BTreeMap::new();
  role_leader.insert(String::from("backend"), Some(String::from("node-a:2552")));

  let state = CurrentClusterState::new(
    vec![member.clone()],
    vec![unreachable.clone()],
    vec![String::from("node-a:2552")],
    Some(String::from("node-a:2552")),
    role_leader.clone(),
  );

  assert_eq!(state.members, vec![member]);
  assert_eq!(state.unreachable, vec![unreachable]);
  assert_eq!(state.seen_by, vec![String::from("node-a:2552")]);
  assert_eq!(state.leader, Some(String::from("node-a:2552")));
  assert_eq!(state.role_leader, role_leader);
}

#[test]
fn current_cluster_state_filters_members_by_data_center_without_losing_status() {
  let east = DataCenter::new("dc-east");
  let west = DataCenter::new("dc-west");
  let east_member = NodeRecord::new_with_identity(
    UniqueAddress::new(Address::new("cluster", "node-a", 2552), 10),
    east.clone(),
    String::from("node-1"),
    NodeStatus::Suspect,
    MembershipVersion::new(1),
    String::from("1.0.0"),
    vec![String::from("backend")],
  );
  let west_member = NodeRecord::new_with_identity(
    UniqueAddress::new(Address::new("cluster", "node-b", 2552), 11),
    west.clone(),
    String::from("node-2"),
    NodeStatus::Up,
    MembershipVersion::new(2),
    String::from("1.0.0"),
    vec![String::from("frontend")],
  );
  let state = CurrentClusterState::new(
    vec![east_member.clone(), west_member],
    vec![east_member.clone()],
    vec![String::from("node-a:2552")],
    Some(String::from("node-a:2552")),
    BTreeMap::new(),
  );

  assert_eq!(state.members_in_data_center(&east), vec![east_member.clone()]);
  assert_eq!(state.unreachable_in_data_center(&east), vec![east_member]);
}
