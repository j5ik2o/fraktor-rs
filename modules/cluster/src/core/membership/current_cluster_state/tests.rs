use alloc::{collections::BTreeMap, string::String, vec};

use crate::core::membership::{CurrentClusterState, MembershipVersion, NodeRecord, NodeStatus};

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
