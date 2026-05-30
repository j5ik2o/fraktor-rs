use alloc::{string::String, vec};

use crate::{
  extension::{ClusterRouterPool, ClusterRouterPoolConfig},
  membership::{MembershipVersion, NodeRecord, NodeStatus},
};

#[test]
fn next_routee_uses_round_robin() {
  let config = ClusterRouterPoolConfig::new(3);
  let mut router = ClusterRouterPool::new(config, vec![String::from("n1"), String::from("n2"), String::from("n3")]);

  assert_eq!(router.next_routee(), Some("n1"));
  assert_eq!(router.next_routee(), Some("n2"));
  assert_eq!(router.next_routee(), Some("n3"));
  assert_eq!(router.next_routee(), Some("n1"));
}

#[test]
fn next_routee_returns_none_when_empty() {
  let config = ClusterRouterPoolConfig::new(1);
  let mut router = ClusterRouterPool::new(config, vec![]);
  assert_eq!(router.next_routee(), None);
}

#[test]
fn replace_routees_from_members_filters_roles_and_local_member() {
  let config =
    ClusterRouterPoolConfig::new(3).with_allow_local_routees(false).with_use_roles(vec![String::from("worker")]);
  let mut router = ClusterRouterPool::new(config, vec![]);

  router.replace_routees_from_members(
    &[
      member("local", NodeStatus::Up, vec!["worker"]),
      member("backend", NodeStatus::Up, vec!["backend"]),
      member("worker-a", NodeStatus::Up, vec!["worker"]),
      member("worker-b", NodeStatus::Leaving, vec!["worker"]),
    ],
    Some("local"),
  );

  assert_eq!(router.routees(), &[String::from("worker-a")]);
  assert_eq!(router.next_routee(), Some("worker-a"));
}

#[test]
fn replace_routees_from_members_applies_max_instances_per_node() {
  let config =
    ClusterRouterPoolConfig::new(5).with_use_roles(vec![String::from("worker")]).with_max_instances_per_node(2);
  let mut router = ClusterRouterPool::new(config, vec![]);

  router.replace_routees_from_members(
    &[
      member("worker-a", NodeStatus::Up, vec!["worker"]),
      member("worker-b", NodeStatus::Suspect, vec!["worker"]),
      member("worker-c", NodeStatus::Dead, vec!["worker"]),
    ],
    None,
  );

  assert_eq!(router.routees(), &[
    String::from("worker-a"),
    String::from("worker-a"),
    String::from("worker-b"),
    String::from("worker-b"),
  ],);
}

fn member(authority: &str, status: NodeStatus, roles: Vec<&str>) -> NodeRecord {
  NodeRecord::new(
    authority.to_string(),
    authority.to_string(),
    status,
    MembershipVersion::new(1),
    String::from("1.0.0"),
    roles.into_iter().map(str::to_string).collect(),
  )
}
