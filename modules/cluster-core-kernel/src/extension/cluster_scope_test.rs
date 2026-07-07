use super::ClusterScope;

#[test]
fn instance_returns_singleton_scope() {
  assert_eq!(ClusterScope::instance(), ClusterScope);
}

#[test]
fn default_equals_instance() {
  assert_eq!(ClusterScope::default(), ClusterScope::instance());
}

#[test]
fn scope_is_copyable() {
  let scope = ClusterScope::instance();
  let copied = scope;
  assert_eq!(scope, copied);
}
