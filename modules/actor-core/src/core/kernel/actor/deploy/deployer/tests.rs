use crate::core::kernel::actor::deploy::{Deploy, Deployer, Scope};

#[test]
fn deployer_registers_and_returns_deployments() {
  let mut deployer = Deployer::new();
  deployer.register("/user/service", Deploy::new().with_scope(Scope::Remote));

  assert_eq!(deployer.deploy_for("/user/service").expect("deploy").scope(), Scope::Remote);
}
