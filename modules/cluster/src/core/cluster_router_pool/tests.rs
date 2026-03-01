use alloc::{string::String, vec};

use crate::core::{ClusterRouterPool, ClusterRouterPoolSettings};

#[test]
fn next_routee_uses_round_robin() {
  let settings = ClusterRouterPoolSettings::new(3);
  let mut router = ClusterRouterPool::new(settings, vec![String::from("n1"), String::from("n2"), String::from("n3")]);

  assert_eq!(router.next_routee(), Some("n1"));
  assert_eq!(router.next_routee(), Some("n2"));
  assert_eq!(router.next_routee(), Some("n3"));
  assert_eq!(router.next_routee(), Some("n1"));
}

#[test]
fn next_routee_returns_none_when_empty() {
  let settings = ClusterRouterPoolSettings::new(1);
  let mut router = ClusterRouterPool::new(settings, vec![]);
  assert_eq!(router.next_routee(), None);
}
