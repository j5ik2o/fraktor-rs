use core::borrow::Borrow;

use crate::actor::{
  Address,
  deploy::{Deploy, Deployer, RemoteScope, Scope},
};

fn assert_remote_node<S: Borrow<Scope>>(scope: S, expected: &Address) {
  match scope.borrow() {
    | Scope::Remote(remote) => assert_eq!(remote.node(), expected),
    | Scope::Local => panic!("deploy should use remote scope"),
  }
}

#[test]
fn deployer_registers_and_returns_deployments() {
  let mut deployer = Deployer::new();
  let node = Address::remote("remote-sys", "10.0.0.1", 2552);
  deployer.register("/user/service", Deploy::new().with_scope(Scope::Remote(RemoteScope::new(node.clone()))));

  let deploy = deployer.deploy_for("/user/service").expect("deploy");
  assert_remote_node(deploy.scope(), &node);
}
