use portable_atomic::Ordering;

use super::IdentityPathRegistry;

#[test]
fn identity_path_registry_starts_with_default_identity() {
  let registry = IdentityPathRegistry::default();

  assert_eq!(registry.path_identity.system_name, "fraktor");
  assert!(registry.path_identity.canonical_host.is_none());
  assert!(registry.path_identity.canonical_port.is_none());
  assert_eq!(registry.temp_counter.load(Ordering::Relaxed), 0);
}
