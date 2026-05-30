use alloc::{string::String, vec};

use crate::extension::{ClusterRouterPool, ClusterRouterPoolConfig};

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
fn from_candidates_distributes_round_robin_within_caps() {
  let config = ClusterRouterPoolConfig::new(5).with_max_instances_per_node(2);
  let candidates = vec![String::from("n1"), String::from("n2"), String::from("n3")];
  let router = ClusterRouterPool::from_candidates(config, &candidates);
  assert_eq!(router.routees(), &[
    String::from("n1"),
    String::from("n2"),
    String::from("n3"),
    String::from("n1"),
    String::from("n2")
  ]);
}

#[test]
fn from_candidates_stops_at_per_node_cap_before_total() {
  let config = ClusterRouterPoolConfig::new(10).with_max_instances_per_node(2);
  let candidates = vec![String::from("n1"), String::from("n2"), String::from("n3")];
  let router = ClusterRouterPool::from_candidates(config, &candidates);
  // 3 nodes * 2 routees per node = 6, capping below the total of 10.
  assert_eq!(router.routees().len(), 6);
  // Placement is balanced: each node hosts exactly max_instances_per_node routees.
  for node in ["n1", "n2", "n3"] {
    let placed = router.routees().iter().filter(|routee| routee.as_str() == node).count();
    assert_eq!(placed, 2);
  }
}

#[test]
fn from_candidates_single_candidate_stops_at_per_node_cap() {
  let config = ClusterRouterPoolConfig::new(10).with_max_instances_per_node(3);
  let router = ClusterRouterPool::from_candidates(config, &[String::from("n1")]);
  // With a single node the per-node cap binds before the total of 10.
  assert_eq!(router.routees(), &[String::from("n1"), String::from("n1"), String::from("n1")]);
}

#[test]
fn from_candidates_with_no_candidates_is_empty() {
  let config = ClusterRouterPoolConfig::new(5);
  let router = ClusterRouterPool::from_candidates(config, &[]);
  assert!(router.routees().is_empty());
}

#[test]
fn from_candidates_one_per_node_then_round_robins() {
  let config = ClusterRouterPoolConfig::new(4).with_max_instances_per_node(1);
  let candidates = vec![String::from("n1"), String::from("n2")];
  let mut router = ClusterRouterPool::from_candidates(config, &candidates);
  // max_instances_per_node = 1 places one routee per node.
  assert_eq!(router.routees(), &[String::from("n1"), String::from("n2")]);
  assert_eq!(router.next_routee(), Some("n1"));
  assert_eq!(router.next_routee(), Some("n2"));
  assert_eq!(router.next_routee(), Some("n1"));
}
