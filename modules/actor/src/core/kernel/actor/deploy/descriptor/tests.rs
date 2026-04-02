use crate::core::kernel::actor::deploy::{Deploy, Scope};

#[test]
fn deploy_builder_preserves_path_and_scope() {
  let deploy = Deploy::new().with_path("/user/service").with_scope(Scope::Remote);

  assert_eq!(deploy.path(), Some("/user/service"));
  assert_eq!(deploy.scope(), Scope::Remote);
}
