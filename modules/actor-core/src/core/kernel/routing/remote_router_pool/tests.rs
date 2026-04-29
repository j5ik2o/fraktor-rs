use alloc::string::String;

use super::super::{ConsistentHashingPool, RandomPool, RemoteRouterPool, RoundRobinPool, SmallestMailboxPool};

#[test]
fn from_round_robin_pool_preserves_pool_contract() {
  let pool = RemoteRouterPool::from(RoundRobinPool::new(3).with_dispatcher(String::from("rr-dispatcher")));

  assert!(matches!(pool, RemoteRouterPool::RoundRobin(_)));
  assert_eq!(pool.nr_of_instances(), 3);
  assert_eq!(pool.router_dispatcher(), "rr-dispatcher");
  assert!(!pool.has_resizer());
  assert!(!pool.use_pool_dispatcher());
  assert!(pool.stop_router_when_all_routees_removed());
}

#[test]
fn from_smallest_mailbox_pool_preserves_pool_contract() {
  let pool = RemoteRouterPool::from(SmallestMailboxPool::new(4).with_dispatcher(String::from("sm-dispatcher")));

  assert!(matches!(pool, RemoteRouterPool::SmallestMailbox(_)));
  assert_eq!(pool.nr_of_instances(), 4);
  assert_eq!(pool.router_dispatcher(), "sm-dispatcher");
}

#[test]
fn from_random_pool_preserves_pool_contract() {
  let pool = RemoteRouterPool::from(RandomPool::new(5).with_dispatcher(String::from("random-dispatcher")));

  assert!(matches!(pool, RemoteRouterPool::Random(_)));
  assert_eq!(pool.nr_of_instances(), 5);
  assert_eq!(pool.router_dispatcher(), "random-dispatcher");
}

#[test]
fn from_consistent_hashing_pool_preserves_pool_contract() {
  let pool =
    RemoteRouterPool::from(ConsistentHashingPool::new(6, |_| 0).with_dispatcher(String::from("consistent-dispatcher")));

  assert!(matches!(pool, RemoteRouterPool::ConsistentHashing(_)));
  assert_eq!(pool.nr_of_instances(), 6);
  assert_eq!(pool.router_dispatcher(), "consistent-dispatcher");
}

#[test]
fn create_router_returns_empty_router_for_every_supported_pool() {
  let pools = [
    RemoteRouterPool::from(RoundRobinPool::new(1)),
    RemoteRouterPool::from(SmallestMailboxPool::new(1)),
    RemoteRouterPool::from(RandomPool::new(1)),
    RemoteRouterPool::from(ConsistentHashingPool::new(1, |_| 0)),
  ];

  for pool in pools {
    assert!(pool.create_router().routees().is_empty());
  }
}
