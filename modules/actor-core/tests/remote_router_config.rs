#![cfg(not(target_os = "none"))]

use core::borrow::Borrow;

use fraktor_actor_core_rs::core::kernel::{
  actor::{
    Address,
    deploy::{RemoteScope, Scope},
  },
  routing::{Pool, RemoteRouterConfig, RouterConfig, SmallestMailboxPool},
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
fn remote_router_config_delegates_pool_contract_to_local_pool() {
  let local_pool = SmallestMailboxPool::new(3).with_dispatcher(String::from("remote-router-dispatcher"));
  let config = RemoteRouterConfig::new(local_pool, vec![node_a(), node_b()]);

  assert_eq!(config.nr_of_instances(), 3);
  assert_eq!(config.router_dispatcher(), "remote-router-dispatcher");
  assert!(!config.has_resizer());
  assert!(!config.use_pool_dispatcher());
  assert_eq!(config.create_router().routees().len(), 0);
}

#[test]
fn deploy_for_routee_index_cycles_remote_nodes() {
  let first = node_a();
  let second = node_b();
  let config = RemoteRouterConfig::new(SmallestMailboxPool::new(3), vec![first.clone(), second.clone()]);

  let deploy_0 = config.deploy_for_routee_index(0);
  let deploy_1 = config.deploy_for_routee_index(1);
  let deploy_2 = config.deploy_for_routee_index(2);

  assert_remote_node(deploy_0.scope(), &first);
  assert_remote_node(deploy_1.scope(), &second);
  assert_remote_node(deploy_2.scope(), &first);
}

#[test]
#[should_panic(expected = "nodes must not be empty")]
fn remote_router_config_rejects_empty_nodes() {
  let _config = RemoteRouterConfig::new(SmallestMailboxPool::new(1), Vec::new());
}

#[test]
fn remote_scope_is_part_of_public_deploy_contract() {
  let node = node_a();
  let scope = Scope::Remote(RemoteScope::new(node.clone()));

  assert_remote_node(&scope, &node);
}
