use core::borrow::Borrow;

use crate::actor::{
  Address,
  deploy::{Deploy, RemoteScope, Scope},
};

fn assert_remote_node<S: Borrow<Scope>>(scope: S, expected: &Address) {
  match scope.borrow() {
    | Scope::Remote(remote) => assert_eq!(remote.node(), expected),
    | Scope::Local => panic!("deploy should use remote scope"),
  }
}

#[test]
fn deploy_builder_preserves_path_and_scope() {
  let node = Address::remote("remote-sys", "10.0.0.1", 2552);
  let deploy = Deploy::new().with_path("/user/service").with_scope(Scope::Remote(RemoteScope::new(node.clone())));

  assert_eq!(deploy.path(), Some("/user/service"));
  assert_remote_node(deploy.scope(), &node);
}
