use crate::actor::{Address, deploy::RemoteScope};

#[test]
fn new_preserves_target_node_address() {
  let node = Address::remote("remote-sys", "10.0.0.2", 2553);

  let scope = RemoteScope::new(node.clone());

  assert_eq!(scope.node(), &node);
}

#[test]
#[should_panic(expected = "RemoteScope requires a remote address")]
fn new_rejects_local_address() {
  let local = Address::local("local-sys");

  let _scope = RemoteScope::new(local);
}
