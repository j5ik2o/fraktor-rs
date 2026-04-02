use crate::core::kernel::actor::deploy::Scope;

#[test]
fn scope_variants_are_distinct() {
  assert_ne!(Scope::Local, Scope::Remote);
}
