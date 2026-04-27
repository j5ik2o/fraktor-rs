use alloc::{string::String, vec::Vec};
use core::borrow::Borrow;

use super::super::{Pool, RemoteRouterConfig, RouterConfig, SmallestMailboxPool};
use crate::core::kernel::actor::{
  Address,
  deploy::{RemoteScope, Scope},
};

fn node_a() -> Address {
  Address::remote("remote-a", "10.0.0.1", 2552)
}

fn node_b() -> Address {
  Address::remote("remote-b", "10.0.0.2", 2553)
}

fn assert_remote_node<S: Borrow<Scope>>(scope: S, expected: &Address) {
  match scope.borrow() {
    | Scope::Remote(remote) => assert_eq!(remote.node(), expected),
    | Scope::Local => panic!("routee deploy should use remote scope"),
  }
}

#[test]
fn new_preserves_pool_contract() {
  let local_pool = SmallestMailboxPool::new(3).with_dispatcher(String::from("remote-router-dispatcher"));
  let config = RemoteRouterConfig::new(local_pool, vec![node_a(), node_b()]);

  assert_eq!(config.nr_of_instances(), 3);
  assert_eq!(config.router_dispatcher(), "remote-router-dispatcher");
  assert!(!config.has_resizer());
  assert!(!config.use_pool_dispatcher());
  assert_eq!(config.create_router().routees().len(), 0);
  assert_eq!(
    config.stop_router_when_all_routees_removed(),
    SmallestMailboxPool::new(3).stop_router_when_all_routees_removed(),
  );
}

#[test]
fn deploy_for_routee_index_cycles_remote_nodes() {
  let first = node_a();
  let second = node_b();
  let config = RemoteRouterConfig::new(SmallestMailboxPool::new(3), vec![first.clone(), second.clone()]);

  assert_remote_node(config.deploy_for_routee_index(0).scope(), &first);
  assert_remote_node(config.deploy_for_routee_index(1).scope(), &second);
  assert_remote_node(config.deploy_for_routee_index(2).scope(), &first);
}

#[test]
#[should_panic(expected = "nodes must not be empty")]
fn new_rejects_empty_nodes() {
  let _config = RemoteRouterConfig::new(SmallestMailboxPool::new(1), Vec::new());
}

#[test]
#[should_panic(expected = "RemoteRouterConfig requires every node to be a remote address with host and port")]
fn new_rejects_local_node_address() {
  let local_address = Address::local("local-system");
  let _config = RemoteRouterConfig::new(SmallestMailboxPool::new(1), vec![node_a(), local_address]);
}

#[test]
fn scope_remote_carries_remote_scope() {
  let node = node_a();
  let scope = Scope::Remote(RemoteScope::new(node.clone()));

  assert_remote_node(&scope, &node);
}
