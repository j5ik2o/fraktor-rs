use crate::actor::{
  Address,
  deploy::{RemoteScope, Scope},
};

#[test]
fn scope_variants_are_distinct() {
  let node = Address::remote("remote-sys", "10.0.0.1", 2552);

  assert_ne!(Scope::Local, Scope::Remote(RemoteScope::new(node)));
}

#[test]
fn remote_scope_preserves_target_node_address() {
  let node = Address::remote("remote-sys", "10.0.0.2", 2553);

  let scope = RemoteScope::new(node.clone());

  assert_eq!(scope.node(), &node);
}
